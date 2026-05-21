using log4net;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.Web.WebView2.Core;
using System;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Text.Json;
using XamlNavigationView = Microsoft.UI.Xaml.Controls.NavigationView;

namespace Obscura_Client;

public sealed partial class MainWindow : Window
{
    private static readonly string HOSTNAME = "obscura-ui";
    private static readonly ILog Log = LogManager.GetLogger(typeof(MainWindow));
    private static readonly ILog WebviewLog = LogManager.GetLogger("Webview");
    public MainWindow()
    {
        InitializeComponent();

        var manager = WinUIEx.WindowManager.Get(this);
        manager.PersistenceId = "MainWindow";
        manager.MinWidth = 500;
        manager.MinHeight = 550;

        string iconPath = Path.Combine(AppContext.BaseDirectory, "Assets/Icon.ico");
        AppWindow.SetIcon(iconPath);

        // Use the modern TitleBar control as the custom title bar
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(AppTitleBar);

        InitializeWebView();

#if DEBUG
        DeveloperNavItem.Visibility = Visibility.Visible;
#endif

        // Select the first navigation item (Connection) by default
        NavView.SelectedItem = NavView.MenuItems[0];
    }

    private async void InitializeWebView()
    {
        await WebView.EnsureCoreWebView2Async();

        await WebView.CoreWebView2.CallDevToolsProtocolMethodAsync("Log.enable", "{}");
        await WebView.CoreWebView2.CallDevToolsProtocolMethodAsync("Runtime.enable", "{}");

        var logEventReceiver = WebView.CoreWebView2.GetDevToolsProtocolEventReceiver("Log.entryAdded");
        logEventReceiver.DevToolsProtocolEventReceived += (s, e) => OnWebviewLogEntry(e.ParameterObjectAsJson);

        var consoleReceiver = WebView.CoreWebView2.GetDevToolsProtocolEventReceiver("Runtime.consoleAPICalled");
        consoleReceiver.DevToolsProtocolEventReceived += (s, e) => OnWebviewConsoleApiCall(e.ParameterObjectAsJson);

        var exceptionReceiver = WebView.CoreWebView2.GetDevToolsProtocolEventReceiver("Runtime.exceptionThrown");
        exceptionReceiver.DevToolsProtocolEventReceived += (s, e) => OnWebviewException(e.ParameterObjectAsJson);

        WebView.CoreWebView2.WebMessageReceived += WebView_WebMessageReceived;
        WebView.CoreWebView2.NewWindowRequested += WebView_NewWindowRequested;
        WebView.CoreWebView2.NavigationStarting += WebView_NavigationStarting;
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
            WebView.CoreWebView2.PostWebMessageAsString(dataJson);
        }
        catch (Exception ex)
        {
            var error = ex.Message;
            var errorJson = WindowsCommandMessage.ReplyError(message.Id, error);
            WebView.CoreWebView2.PostWebMessageAsString(errorJson);
        }
    }

    private void WebView_NewWindowRequested(CoreWebView2 sender, CoreWebView2NewWindowRequestedEventArgs args)
    {
        args.Handled = true;
        OpenInDefaultBrowser(args.Uri);
    }

    private static void WebView_NavigationStarting(CoreWebView2 sender, CoreWebView2NavigationStartingEventArgs args)
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

    private static void HandleObscuraUrl(Uri uri)
    {
        switch (uri.AbsolutePath)
        {
            case "/account":
                OsStatus.Instance.SetNavigationView(NavigationView.Account);
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

    private void TitleBar_PaneToggleRequested(TitleBar sender, object args)
    {
        NavView.IsPaneOpen = !NavView.IsPaneOpen;
    }

    private void NavView_SelectionChanged(XamlNavigationView sender, NavigationViewSelectionChangedEventArgs args)
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
        public string Data { get; set; } = "";
    }
}
