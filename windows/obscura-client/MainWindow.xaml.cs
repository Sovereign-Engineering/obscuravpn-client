using System;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Text.Json;
using System.Threading.Tasks;
using log4net;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.Web.WebView2.Core;
using Windows.Win32;
using Windows.Win32.Foundation;
using Windows.Win32.UI.WindowsAndMessaging;
using WinUIEx.Messaging;
using XamlNavigationView = Microsoft.UI.Xaml.Controls.NavigationView;

namespace Obscura_Client;

public sealed class NativeUiError
{
    public string Message { get; set; } = "";
    public bool Fatal { get; set; } = true;
}

public sealed partial class MainWindow : Window, INotifyPropertyChanged
{
    public event PropertyChangedEventHandler? PropertyChanged;

    // When non-empty, an overlay covering the WebView lists these errors.
    public ObservableCollection<NativeUiError> NativeUiErrors { get; } = [];

    public Visibility NativeUiErrorVisibility =>
        NativeUiErrors.Count > 0 ? Visibility.Visible : Visibility.Collapsed;

    public Visibility NativeUiErrorDismissVisibility =>
        NativeUiErrors.Count > 0 && NativeUiErrors.All(e => !e.Fatal)
            ? Visibility.Visible : Visibility.Collapsed;

    internal void AddNativeUiError(string error, bool fatal = true)
    {
        var item = new NativeUiError { Message = error, Fatal = fatal };
        if (DispatcherQueue.HasThreadAccess)
        {
            NativeUiErrors.Add(item);
        }
        else
        {
            DispatcherQueue.TryEnqueue(() => NativeUiErrors.Add(item));
        }
    }

    void OnDismissErrorsClick(object sender, RoutedEventArgs e)
    {
        foreach (var error in NativeUiErrors.Where(err => !err.Fatal).ToList())
        {
            NativeUiErrors.Remove(error);
        }
    }

#if !DEBUG
    static readonly string HOSTNAME = "obscura-ui";
#endif
    static readonly ILog Log = LogManager.GetLogger(typeof(MainWindow));
    static readonly ILog WebviewLog = LogManager.GetLogger("Webview");
    // References to CoreWebView2DevToolsProtocolEventReceiver MUST be held
    // Otherwise, undefined behaviour including exceptions that crash such as memory access violation may occur
    CoreWebView2DevToolsProtocolEventReceiver? _logEventReceiver;
    CoreWebView2DevToolsProtocolEventReceiver? _consoleReceiver;
    CoreWebView2DevToolsProtocolEventReceiver? _exceptionReceiver;

    readonly WindowMessageMonitor _msgMonitor;
    // In case cold launch is from a `/payment-succeeded` URI protocol launch
    readonly TaskCompletionSource _webUIReady = new();
    readonly StatusSubscriber _statusSubscriber = new();

    const uint WM_CLOSE = 0x0010;

    public MainWindow()
    {
        InitializeComponent();

        var manager = WinUIEx.WindowManager.Get(this);
        manager.PersistenceId = "MainWindow";
        manager.MinWidth = 500;
        manager.MinHeight = 550;

        string iconPath = Path.Combine(AppContext.BaseDirectory, "Assets/Icon.ico");
        AppWindow.SetIcon(iconPath);

        // The error overlay's visibility is a OneWay x:Bind on NativeUiErrorVisibility, which doesn't
        // observe the collection itself; fire event whenever the error set changes.
        NativeUiErrors.CollectionChanged += (_, _) =>
        {
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(NativeUiErrorVisibility)));
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(NativeUiErrorDismissVisibility)));
        };
        // Use the modern TitleBar control as the custom title bar
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(AppTitleBar);
        InitializeWebView();
#if DEBUG
        DeveloperNavItem.Visibility = Visibility.Visible;
#endif
        // Select the first navigation item (Connection) by default
        NavView.SelectedItem = NavView.MenuItems[0];

        _statusSubscriber.StatusChanged += OnStatusChanged;
        _statusSubscriber.Start();

        // Subclass WM_CLOSE *before* WinUI sees it. The standard AppWindow.Closing +
        // args.Cancel pattern destabilizes WebView2: each cancel-close leaves it partially
        // torn down, and the next show/focus event then NULL-derefs inside
        // Microsoft.Web.WebView2.Core.dll. Intercepting at the Win32 level keeps WinUI's
        // close-cancel machinery completely out of the loop — WebView2 never learns a
        // close was attempted, so its state stays clean across hide/show cycles.
        _msgMonitor = new WindowMessageMonitor(this);
        _msgMonitor.WindowMessageReceived += OnWindowMessageReceived;

        Closed += OnClosed;
    }

    private void OnStatusChanged(NeStatus status)
    {
        bool showNav = status.AccountId != null && !status.InNewAccountFlow;
        DispatcherQueue.TryEnqueue(() => {
            NavView.IsPaneVisible = showNav;
            AppTitleBar.IsPaneToggleButtonVisible = showNav;
        });
    }

    private void OnClosed(object sender, WindowEventArgs args)
    {
        // Window.Close() destroys the HWND but does not release the WebView2's CoreWebView2.
        // The CoreWebView2 + DevTools event receivers are COM RCWs that keep the STA apartment
        // alive, which in turn keeps the process alive. Explicitly closing the WebView2 releases
        // those references so the process can actually exit.
        try
        {
            WebView.CoreWebView2.WebMessageReceived -= WebView_WebMessageReceived;
            WebView.CoreWebView2.NewWindowRequested -= WebView_NewWindowRequested;
            WebView.CoreWebView2.NavigationStarting -= WebView_NavigationStarting;
            WebView.Close();
        }
        catch (Exception ex) { Log.Warn($"WebView2 close failed: {ex.Message}"); }
    }

    private async void InitializeWebView()
    {
        try {
            await WebView.EnsureCoreWebView2Async();
        } catch (Exception ex) {
            Log.Error($"WebView.EnsureCoreWebView2Async failed: {ex.Message}");
            AddNativeUiError(ex.ToString());
            return;
        }

        await WebView.CoreWebView2.CallDevToolsProtocolMethodAsync("Log.enable", "{}");
        await WebView.CoreWebView2.CallDevToolsProtocolMethodAsync("Runtime.enable", "{}");

        _logEventReceiver = WebView.CoreWebView2.GetDevToolsProtocolEventReceiver("Log.entryAdded");
        _logEventReceiver.DevToolsProtocolEventReceived += (s, e) => OnWebviewLogEntry(e.ParameterObjectAsJson);

        _consoleReceiver = WebView.CoreWebView2.GetDevToolsProtocolEventReceiver("Runtime.consoleAPICalled");
        _consoleReceiver.DevToolsProtocolEventReceived += (s, e) => OnWebviewConsoleApiCall(e.ParameterObjectAsJson);

        _exceptionReceiver = WebView.CoreWebView2.GetDevToolsProtocolEventReceiver("Runtime.exceptionThrown");
        _exceptionReceiver.DevToolsProtocolEventReceived += (s, e) => OnWebviewException(e.ParameterObjectAsJson);

        WebView.CoreWebView2.WebMessageReceived += WebView_WebMessageReceived;
        WebView.CoreWebView2.NewWindowRequested += WebView_NewWindowRequested;
        WebView.CoreWebView2.NavigationStarting += WebView_NavigationStarting;
        WebView.CoreWebView2.NavigationCompleted += OnInitialNavigationCompleted;
#if DEBUG
        WebView.CoreWebView2.Navigate($"http://localhost:{DevServer.PORT}/");
#else
            WebView.CoreWebView2.Settings.AreHostObjectsAllowed = true;
            WebView.CoreWebView2.Settings.AreDevToolsEnabled = false;
            WebView.CoreWebView2.Settings.AreDefaultContextMenusEnabled = false;
            WebView.CoreWebView2.Settings.AreBrowserAcceleratorKeysEnabled = false;
            string webUIPath = Path.Combine(AppContext.BaseDirectory, "Assets/webui");
            string indexPath = Path.Combine(webUIPath, "index.html");
            Log.Info($"WebUI directory: {webUIPath} (exists: {Directory.Exists(webUIPath)})");
            Log.Info($"WebUI index.html: {indexPath} (exists: {File.Exists(indexPath)})");
            if (Directory.Exists(webUIPath))
            {
                var entries = Directory.GetFileSystemEntries(webUIPath);
                Log.Info($"WebUI directory contains {entries.Length} entries: {string.Join(", ", entries.Select(Path.GetFileName))}");
            }
            WebView.CoreWebView2.SetVirtualHostNameToFolderMapping(
                HOSTNAME, webUIPath, CoreWebView2HostResourceAccessKind.Allow);
            WebView.CoreWebView2.Navigate($"https://{HOSTNAME}/index.html");
#endif
    }

    private void OnInitialNavigationCompleted(CoreWebView2 sender, CoreWebView2NavigationCompletedEventArgs args)
    {
        if (args.IsSuccess)
        {
            sender.NavigationCompleted -= OnInitialNavigationCompleted;
            _webUIReady.TrySetResult();
        } else
        {
            Log.Error($"webview initial navigation failed {args.WebErrorStatus}");
        }
    }

    private void OnWindowMessageReceived(object? sender, WindowMessageEventArgs e)
    {
        if (e.Message.MessageId == WM_CLOSE)
        {
            // Hide at the HWND level. AppWindow.Hide() still nudges parts of WinUI's
            // window state machine that interact poorly with WebView2.
            PInvoke.ShowWindow(GetWindowHandle(), SHOW_WINDOW_CMD.SW_HIDE);
            e.Handled = true;
            e.Result = 0;
        }
    }

    internal HWND GetWindowHandle()
    {
        return (HWND)WinRT.Interop.WindowNative.GetWindowHandle(this);
    }

    class WindowsCommandMessage
    {
        const string prefix = "windows/";

        public ulong Id { get; set; }
        public string? Error { get; set; }
        public string? Data { get; set; }
        public static string ReplyError(ulong id, string? error)
        {
            return new WindowsCommandMessage
            {
                Id = id,
                Error = error ?? "other"
            }.Reply();
        }

        public static string ReplyData(ulong id, string data)
        {
            return new WindowsCommandMessage
            {
                Id = id,
                Data = data
            }.Reply();
        }
        string Reply()
        {
            var response = JsonSerializer.Serialize(this, JsonConfig.Options);
            return $"{prefix}{response}";
        }
    }

    private static void OnWebviewLogEntry(string json)
    {
        try
        {
            using var doc = JsonDocument.Parse(json);
            var entry = doc.RootElement.GetProperty("entry");
            var level = entry.TryGetProperty("level", out var lv) ? lv.GetString() : null;
            var text = entry.TryGetProperty("text", out var tv) ? tv.GetString() ?? "" : "";
            var source = entry.TryGetProperty("source", out var sv) ? sv.GetString() : null;
            var url = entry.TryGetProperty("url", out var uv) ? uv.GetString() : null;
            var prefix = source != null ? $"[{source}] " : "";
            var suffix = url != null ? $" ({url})" : "";
            var msg = prefix + text + suffix;
            switch (level)
            {
                case "error": WebviewLog.Error(msg); break;
                case "warning": WebviewLog.Warn(msg); break;
                case "info": WebviewLog.Info(msg); break;
                case "verbose": WebviewLog.Debug(msg); break;
                default: WebviewLog.Info(msg); break;
            }
        }
        catch (Exception ex)
        {
            WebviewLog.Warn($"failed to parse Log.entryAdded: {ex.Message}: {json}");
        }
    }

    private static void OnWebviewConsoleApiCall(string json)
    {
        try
        {
            using var doc = JsonDocument.Parse(json);
            var root = doc.RootElement;
            var type = root.TryGetProperty("type", out var tv) ? tv.GetString() : null;
            var message = root.TryGetProperty("args", out var av) && av.ValueKind == JsonValueKind.Array
                ? string.Join(" ", av.EnumerateArray().Select(FormatConsoleArg))
                : "";
            switch (type)
            {
                case "error":
                case "assert":
                    WebviewLog.Error(message);
                    break;
                case "warning":
                    WebviewLog.Warn(message);
                    break;
                case "debug":
                case "trace":
                    WebviewLog.Debug(message);
                    break;
                default:
                    WebviewLog.Info(message);
                    break;
            }
        }
        catch (Exception ex)
        {
            WebviewLog.Warn($"failed to parse Runtime.consoleAPICalled: {ex.Message}: {json}");
        }
    }

    private static void OnWebviewException(string json)
    {
        try
        {
            using var doc = JsonDocument.Parse(json);
            var details = doc.RootElement.GetProperty("exceptionDetails");
            var text = details.TryGetProperty("text", out var tv) ? tv.GetString() ?? "" : "";
            var description = details.TryGetProperty("exception", out var ex) && ex.TryGetProperty("description", out var dv)
                ? dv.GetString()
                : null;
            WebviewLog.Error(description ?? text);
        }
        catch (Exception ex)
        {
            WebviewLog.Warn($"failed to parse Runtime.exceptionThrown: {ex.Message}: {json}");
        }
    }

    private static string FormatConsoleArg(JsonElement arg)
    {
        if (arg.TryGetProperty("value", out var v))
        {
            return v.ValueKind == JsonValueKind.String ? v.GetString() ?? "" : v.GetRawText();
        }
        if (arg.TryGetProperty("description", out var d))
        {
            return d.GetString() ?? "";
        }
        return arg.GetRawText();
    }

    private async void WebView_WebMessageReceived(CoreWebView2 sender, CoreWebView2WebMessageReceivedEventArgs e)
    {
        string messageJson = e.WebMessageAsJson;

        BridgeMessage? message;
        try
        {
            message = JsonSerializer.Deserialize<BridgeMessage>(messageJson, JsonConfig.Options);
        }
        catch
        {
            Log.Warn($"Failed to parse bridge message: {messageJson}");
            return;
        }

        if (message == null)
        {
            Log.Warn($"Got null message: {messageJson}");
            return;
        }

        try
        {
            var responseJson = await InvokeCommand.Parse(message.Data).RunAsync();
            var dataJson = WindowsCommandMessage.ReplyData(message.Id, responseJson);
            WebView?.CoreWebView2?.PostWebMessageAsString(dataJson);
        }
        catch (Exception ex)
        {
            var error = ex.Message;
            var errorJson = WindowsCommandMessage.ReplyError(message.Id, error);
            WebView?.CoreWebView2?.PostWebMessageAsString(errorJson);
        }
    }

    private void WebView_NewWindowRequested(CoreWebView2 sender, CoreWebView2NewWindowRequestedEventArgs args)
    {
        args.Handled = true;
        OpenInDefaultBrowser(args.Uri);
    }

    private void WebView_NavigationStarting(CoreWebView2 sender, CoreWebView2NavigationStartingEventArgs args)
    {
        if (!Uri.TryCreate(args.Uri, UriKind.Absolute, out var uri))
        {
            return;
        }

        if (uri.Scheme == "http" || uri.Scheme == "https")
        {
#if DEBUG
            if (uri.Authority == $"localhost:{DevServer.PORT}")
            {
                return;
            }
#else
            if (uri.Host == HOSTNAME)
            {
                return;
            }
#endif
            args.Cancel = true;
            OpenInDefaultBrowser(uri.ToString());
            return;
        }

        if (uri.Scheme == "obscuravpn")
        {
            args.Cancel = true;
            HandleObscuraUrl(uri);
            return;
        }

        args.Cancel = true;
        Log.Warn($"Blocked navigation to unsupported scheme: {args.Uri}");
    }

    internal async void HandleObscuraUrl(Uri uri)
    {
        Log.Info("HandleObscuraUrl called");
        switch (uri.AbsolutePath)
        {
            case "/open":
                break;
            case "/account":
                SelectNavigationView(NavigationView.Account);
                break;
            case "/location":
                SelectNavigationView(NavigationView.Location);
                break;
            case "/payment-succeeded":
                await _webUIReady.Task;
                await WebView.CoreWebView2.ExecuteScriptAsync("window.dispatchEvent(new CustomEvent('paymentSucceeded'))");
                break;
            default:
                Log.Warn($"Unhandled obscuravpn path: {uri.AbsolutePath}");
                break;
        }
    }

    private static void OpenInDefaultBrowser(string url)
    {
        try
        {
            Process.Start(new ProcessStartInfo
            {
                FileName = url,
                UseShellExecute = true,
            });
        }
        catch (Exception ex)
        {
            Log.Warn($"Failed to open URL in default browser: {url}: {ex.Message}");
        }
    }

    internal void TitleBar_PaneToggleRequested(TitleBar _, object _1)
    {
        NavView.IsPaneOpen = !NavView.IsPaneOpen;
    }

    internal void SelectNavigationView(NavigationView view)
    {
        var item = NavView.MenuItems.Concat(NavView.FooterMenuItems)
            .OfType<NavigationViewItem>()
            .FirstOrDefault(i => i.Tag is int tag && tag == (int)view);
        if (item == null)
        {
            Log.Warn($"No navigation pane item for view: {view}");
            return;
        }
        NavView.SelectedItem = item;
    }

#pragma warning disable CA1822
    void NavView_SelectionChanged(XamlNavigationView _, NavigationViewSelectionChangedEventArgs args)
#pragma warning restore CA1822
    {
        if (args.SelectedItem is NavigationViewItem item && item.Tag is int tagValue
            && typeof(NavigationView).IsEnumDefined(item.Tag))
        {
            OsStatus.Instance.SetNavigationView((NavigationView)tagValue);
        }
    }

    private class BridgeMessage
    {
        public ulong Id { get; set; }
        public required string Data { get; set; }
    }
}
