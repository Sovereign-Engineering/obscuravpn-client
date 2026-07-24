using System;
using System.Collections.Generic;
using System.IO;
using System.Threading;
using System.Threading.Tasks;
using log4net;
using log4net.Appender;
using log4net.Config;
using log4net.Layout;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.Windows.AppLifecycle;
using Microsoft.Windows.AppNotifications;
using Microsoft.Windows.AppNotifications.Builder;
using Obscura_Client.NotifyIcon;
using Windows.Win32;
using Windows.Win32.Foundation;
using Windows.Win32.System.Com;
using Windows.Win32.System.DataExchange;
using Windows.Win32.UI.WindowsAndMessaging;

namespace Obscura_Client;

/// <summary>
/// Provides application-specific behavior to supplement the default Application class.
/// </summary>
public partial class App : Application
{
    static readonly ILog Log = LogManager.GetLogger(typeof(App));
    public new static App Current => (App)Application.Current;
    MainWindow? _window;
    // Completed once OnLaunched creates the window; lets activations that arrive earlier wait for it.
    readonly TaskCompletionSource<MainWindow> _windowReady = new(TaskCreationOptions.RunContinuationsAsynchronously);
    NotifyIconManager? _notifyIcon;
    DispatcherQueue? _uiDispatcher;

    // %LOCALAPPDATA%\Obscura
    internal static string ObscuraLocalAppDir => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
        "Obscura");

    /// <summary>
    /// Initializes the singleton application object.  This is the first line of authored code
    /// executed, and as such is the logical equivalent of main() or WinMain().
    /// </summary>
    public App()
    {
        InitializeComponent();
#if DEBUG
        DevServer.Start();
        AppDomain.CurrentDomain.ProcessExit += (s, e) => DevServer.Stop();
        UnhandledException += (s, e) => DevServer.Stop();
#endif
        UnhandledException += OnUnhandledException;
    }

    void OnUnhandledException(object sender, Microsoft.UI.Xaml.UnhandledExceptionEventArgs e)
    {
        Log.Error("Unhandled Exception", e.Exception);
        try
        {
            _window?.AddNativeUiError($"Unhandled exception: {e.Exception?.ToString() ?? e.Message}", fatal: false);
        }
        catch (Exception ex)
        {
            Log.Warn($"failed to surface unhandled exception: {ex.Message}");
        }
    }

    /// <summary>
    /// Invoked when the application is launched.
    /// </summary>
    /// <param name="args">Details about the launch request and process.</param>
    private static void ConfigureLogging()
    {
        GlobalContext.Properties["pid"] = Environment.ProcessId;

        var layout = new SerializedLayout();
        layout.AddArrangement(new log4net.Layout.Arrangements.DefaultArrangement());
        layout.AddMember("pid");
        layout.ActivateOptions();

        var traceAppender = new TraceAppender { Layout = layout };
        traceAppender.ActivateOptions();

        var logDir = Path.Combine(ObscuraLocalAppDir, "logs");
        Directory.CreateDirectory(logDir);

        var fileAppender = new RollingFileAppender
        {
            File = Path.Combine(logDir, "client.ndjson"),
            AppendToFile = true,
            RollingStyle = RollingFileAppender.RollingMode.Size,
            MaxSizeRollBackups = 24,
            MaximumFileSize = "10MB",
            StaticLogFileName = true,
            Layout = layout,
            // The default ExclusiveLock silently drops all log output of a second app instance
            LockingModel = new FileAppender.MinimalLock(),
        };
        fileAppender.ActivateOptions();

        BasicConfigurator.Configure(traceAppender, fileAppender);
    }

    protected override void OnLaunched(LaunchActivatedEventArgs launchArgs)
    {
        _uiDispatcher = DispatcherQueue.GetForCurrentThread();
        _window = new MainWindow();
        _windowReady.TrySetResult(_window);
        _notifyIcon = new NotifyIconManager(this, _uiDispatcher);

        AppNotificationManager.Default.NotificationInvoked += (s, a) => HandleNotification(a);
        Log.Info("registering notification manager");
        try
        {
            AppNotificationManager.Default.Register();
        } catch (Exception ex)
        {
            Log.Error("Failed to register notifications", ex);
            if (PackageIdentity.IsPackagedProcess()) {
                throw;
            }
        }
        

        var args = AppInstance.GetCurrent().GetActivatedEventArgs();
        if (args.Kind == ExtendedActivationKind.AppNotification)
        {
            var notificationArgs = (AppNotificationActivatedEventArgs)args.Data;
            HandleNotification(notificationArgs);
        } else if (args.Kind != ExtendedActivationKind.StartupTask) {
            ShowMainWindow();
        }
        HandleActivation(args);
        ShowFirstRunNotification();
        _ = LoginItem.RefreshStatusAsync();
    }

    static void ShowFirstRunNotification()
    {
        Log.Info("checking first-run state");
        try
        {
            if (!ClientSettings.IsFirstRun)
            {
                Log.Info("not first run; skipping notification");
                return;
            }
            Log.Info("notification manager registered");
            AppNotificationManager.Default.Show(new AppNotificationBuilder()
                .AddText("Obscura VPN is running in the tray.")
                .AddText("You can find it by pressing the arrow (^).")
                .AddText("Obscura VPN will continue running in status notification area if the window is closed.")
                .BuildNotification());
            ClientSettings.SetFirstRunCompleted();
            Log.Info("first-run notification shown");
        }
        catch (Exception ex)
        {
            Log.Warn($"first-run notification failed: {ex}");
        }
    }

    private async void HandleActivation(AppActivationArguments activationArgs)
    {
        if (activationArgs.Kind == ExtendedActivationKind.Protocol
            && activationArgs.Data is Windows.ApplicationModel.Activation.IProtocolActivatedEventArgs protocolArgs)
        {
            Log.Info($"handling protocol activation: {protocolArgs.Uri}");
            var window = await _windowReady.Task;
            window.DispatcherQueue.TryEnqueue(() => window.HandleObscuraUrl(protocolArgs.Uri));
        }
    }

    private void HandleNotification(AppNotificationActivatedEventArgs _) => ShowMainWindow();

    internal void SelectNavigationView(NavigationView view)
    {
        _window?.SelectNavigationView(view);
    }

    internal void ApplyColorScheme(ElementTheme theme)
    {
        _window?.ApplyColorScheme(theme);
    }

    internal void ShowMainWindow()
    {
        Log.Info("activating main window");
        if (_window != null)
        {
            var hwnd = _window.GetWindowHandle();
            PInvoke.ShowWindow(hwnd, SHOW_WINDOW_CMD.SW_NORMAL);
            PInvoke.SetForegroundWindow(hwnd);
            Log.Info("activated main window");
        } else
        {
            Log.Warn("main window not created yet");
        }
    }

    /// <summary>
    /// Exit because the session is ending or the installer asked us to close. Unlike
    /// RequestQuit, the tunnel is left alone: the service owns it.
    /// </summary>
    internal void ExitForShutdown()
    {
        _notifyIcon?.Close();
        Exit();
    }

    /// <summary>
    /// User-initiated quit path from the tray menu. Disconnects the tunnel, disposes the
    /// tray, then terminates the process. Errors during disconnect are logged but do not
    /// block quitting — getting the user out is more important than a clean disconnect.
    /// </summary>
    internal async void RequestQuit()
    {
        _window?.Close();
        _notifyIcon?.Close();
        try
        {
            // Bounded wait: if the service has accepted the pipe but is wedged before
            // replying, the await would otherwise hang forever and trap the user in a
            // dead tray. Cancellation surfaces as an exception, handled below.
            using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(3));
            await IPCCommand.RunWithArgAsync(new SetTunnelArgs { Active = false }, cts.Token);
        }
        catch (Exception ex)
        {
            Log.Warn($"disconnect on quit failed: {ex.Message}");
        }
        Exit();
    }

    /// <summary>
    /// Before booting the WinUI runtime, we claim the single-instance key. A redundant launch hands
    /// its activation to the primary instance and exits immediately; One UI process ever runs.
    /// </summary>
    [STAThread]
    static void Main()
    {
        ConfigureLogging();
        Log.Info("App instance launched; Logging initialized");
        try {
            WinRT.ComWrappersSupport.InitializeComWrappers();
            Log.Info("COM wrappers initialized");
        } catch (Exception ex)
        {
            Log.Error($"Failed to initialize COM wrappers. Not pumping COM. {ex}");
        }

        if (DecideRedirection())
        {
            // Activation has been handed to the primary instance
            return;
        }

        Start((p) =>
        {
            var context = new DispatcherQueueSynchronizationContext(DispatcherQueue.GetForCurrentThread());
            SynchronizationContext.SetSynchronizationContext(context);
            _ = new App();
        });
    }

    /// <summary>
    /// Claims the single-instance key. Returns true if this process is a redundant launch whose
    /// activation has been redirected to the primary instance (and should therefore exit).
    /// </summary>
    private static bool DecideRedirection()
    {
        var keyInstance = AppInstance.FindOrRegisterForKey("primary");
        if (keyInstance.IsCurrent)
        {
            // Handle redirect activations of subsequent launches
            keyInstance.Activated += OnRedirectActivated;
            return false;
        }
        // Subsequent launches will redirect activation to the primary instance
        var activationArgs = AppInstance.GetCurrent().GetActivatedEventArgs();
        Log.Info($"Secondary instance; redirecting activation (kind={activationArgs.Kind}) to primary instance (pid {keyInstance.ProcessId})");
        RedirectActivationTo(activationArgs, keyInstance);
        return true;
    }

    private static void RedirectActivationTo(AppActivationArguments activationArgs, AppInstance keyInstance)
    {
        // A single-thread apartment (STA) must pump messages while idle to avoid jamming up window broadcasts
        // The redirect activation is waited using a method that continues dispatching messages
        using var redirectComplete = new ManualResetEvent(false);
        var redirectTimeout = TimeSpan.FromSeconds(5);
        using var cts = new CancellationTokenSource(redirectTimeout);
        Task.Run(() =>
        {
            try
            {
                keyInstance.RedirectActivationToAsync(activationArgs).AsTask(cts.Token).GetAwaiter().GetResult();
                Log.Info("Redirected activation to primary instance");
            }
            catch (OperationCanceledException)
            {
                Log.Error($"Failed to activate existing instance; timed out after {redirectTimeout}.");
            }
            catch (Exception ex)
            {
                Log.Error("Failed to activate existing instance; using WM_COPYDATA fallback", ex);
                try
                {
                    FallbackActivatePrimary(activationArgs, keyInstance);
                }
                catch (Exception fallbackEx)
                {
                    Log.Error("WM_COPYDATA fallback failed", fallbackEx);
                }
            }
            finally
            {
                redirectComplete.Set();
            }
        });
        var handle = new HANDLE(redirectComplete.SafeWaitHandle.DangerousGetHandle());
        PInvoke.CoWaitForMultipleObjects((uint)CWMO_FLAGS.CWMO_DEFAULT, PInvoke.INFINITE, [handle], out _);
    }

    // "OBS"; distinguishes our activation hand-off from other WM_COPYDATA traffic
    internal const nuint OBS_ACTIVATION_TAG = 0x4F4253;

    /// <summary>
    /// RedirectActivationToAsync marshals IAppActivationArguments via WinRT metadata resolution,
    /// which fails (0x80040155) on Windows 10 for self-contained deployments with sparse package
    /// identity: https://github.com/microsoft/WindowsAppSDK/issues/3439#issuecomment-4970200486.
    /// Hand the payload to the primary instance via WM_COPYDATA instead;
    /// MainWindow replies 1 when it accepts.
    /// </summary>
    private static void FallbackActivatePrimary(AppActivationArguments activationArgs, AppInstance keyInstance)
    {
        var payload = activationArgs.Kind == ExtendedActivationKind.Protocol
            && activationArgs.Data is Windows.ApplicationModel.Activation.IProtocolActivatedEventArgs protocolArgs
            ? protocolArgs.Uri.ToString()
            : "";

        var candidates = new List<HWND>();
        PInvoke.EnumWindows((hwnd, _) =>
        {
            if (GetWindowPid(hwnd) == keyInstance.ProcessId)
            {
                candidates.Add(hwnd);
            }
            return true;
        }, 0);

        PInvoke.AllowSetForegroundWindow(keyInstance.ProcessId);
        foreach (var hwnd in candidates)
        {
            if (SendActivationPayload(hwnd, payload))
            {
                Log.Info("Activated primary instance via WM_COPYDATA fallback");
                return;
            }
        }
        Log.Error($"WM_COPYDATA fallback not accepted by any of {candidates.Count} windows of pid {keyInstance.ProcessId}");
    }

    /// <summary>
    /// Isolates call to unsafe method GetWindowThreadProcessId
    /// </summary>
    private static unsafe uint GetWindowPid(HWND hwnd)
    {
        uint pid;
        // SAFETY: &pid is not null; points to the stack-allocated variable pid
        var _ = PInvoke.GetWindowThreadProcessId(hwnd, &pid);
        return pid;
    }

    /// <summary>
    /// Isolates WM_COPYDATA marshaling: building a COPYDATASTRUCT requires
    /// pinning the payload string and passing raw addresses.
    /// </summary>
    private static unsafe bool SendActivationPayload(HWND hwnd, string payload)
    {
        fixed (char* payloadPtr = payload)
        {
            var copyData = new COPYDATASTRUCT
            {
                dwData = OBS_ACTIVATION_TAG,
                cbData = (uint)(payload.Length * sizeof(char)),
                lpData = payloadPtr,
            };
            nuint result = 0;
            // SAFETY: SendMessageTimeout is synchronous, so the fixed pin and the stack-allocated
            // copyData/result outlive the call; the OS copies the buffer into the receiving
            // process, so nothing is referenced after return.
            PInvoke.SendMessageTimeout(hwnd, PInvoke.WM_COPYDATA, 0, (nint)(&copyData),
                SEND_MESSAGE_TIMEOUT_FLAGS.SMTO_ABORTIFHUNG, 5000, &result);
            return result == 1;
        }
    }

    private static async void OnRedirectActivated(object? sender, AppActivationArguments args)
    {
        Log.Info($"received redirected activation (kind={args.Kind})");
        await Current._windowReady.Task;
        Current.ShowMainWindow();
        Current.HandleActivation(args);
    }
}
