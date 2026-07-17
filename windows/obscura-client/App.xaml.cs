using System;
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

// To learn more about WinUI, the WinUI project structure,
// and more about our project templates, see: http://aka.ms/winui-project-info.

namespace Obscura_Client;

/// <summary>
/// Provides application-specific behavior to supplement the default Application class.
/// </summary>
public partial class App : Application
{
    static readonly ILog Log = LogManager.GetLogger(typeof(App));
    public new static App Current => (App)Application.Current;
    MainWindow? _window;
    NotifyIconManager? _notifyIcon;
    DispatcherQueue? _uiDispatcher;
    // A redirect activation can arrive before _uiDispatcher is set
    // OnRedirectActivated waits on _uiDispatcherReady
    static readonly TaskCompletionSource<DispatcherQueue> _uiDispatcherReady = new();

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
        UnhandledException += (s, e) => _notifyIcon?.Close();
    }

    /// <summary>
    /// Invoked when the application is launched.
    /// </summary>
    /// <param name="args">Details about the launch request and process.</param>
    private static void ConfigureLogging()
    {
        var layout = new SerializedLayout();
        layout.AddArrangement(new log4net.Layout.Arrangements.DefaultArrangement());
        layout.ActivateOptions();

        var traceAppender = new TraceAppender { Layout = layout };
        traceAppender.ActivateOptions();

        var logDir = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "Obscura VPN", "logs");
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
        };
        fileAppender.ActivateOptions();

        BasicConfigurator.Configure(traceAppender, fileAppender);
    }

    protected override void OnLaunched(LaunchActivatedEventArgs launchArgs)
    {
        _uiDispatcher = DispatcherQueue.GetForCurrentThread();
        _uiDispatcherReady.TrySetResult(_uiDispatcher);
        _notifyIcon = new NotifyIconManager(this, _uiDispatcher);
        _window = new MainWindow();

        AppNotificationManager.Default.NotificationInvoked += (s, a) => HandleNotification(a);
        Log.Info("registering notification manager");
        AppNotificationManager.Default.Register();

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

    private static void HandleActivation(AppActivationArguments activationArgs)
    {
        if (activationArgs.Kind == ExtendedActivationKind.Protocol
            && activationArgs.Data is Windows.ApplicationModel.Activation.IProtocolActivatedEventArgs protocolArgs)
        {
            Log.Info($"handling protocol activation: {protocolArgs.Uri}");
            Current?._window?.HandleObscuraUrl(protocolArgs.Uri);
        }
    }

    private void HandleNotification(AppNotificationActivatedEventArgs _) => ShowMainWindow();

    internal void SelectNavigationView(NavigationView view)
    {
        _window?.SelectNavigationView(view);
    }

    internal void ShowMainWindow()
    {
        Log.Info("activating main window");
        if (_window == null)
        {
            Log.Warn("creating new main window");
            _window = new MainWindow();
        }
        _window.DispatcherQueue.TryEnqueue(_window.Activate);
        PInvoke.SetForegroundWindow(_window.GetWindowHandle());
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
        WinRT.ComWrappersSupport.InitializeComWrappers();

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
        RedirectActivationTo(activationArgs, keyInstance);
        return true;
    }

    private static void RedirectActivationTo(AppActivationArguments activationArgs, AppInstance keyInstance)
    {
        // A single-thread apartment (STA) must pump messages while idle to avoid jamming up window broadcasts
        // The redirect activation is waited using a method that continues dispatching messages
        using var redirectComplete = new ManualResetEvent(false);
        var redirectTimeout = TimeSpan.FromSeconds(32);
        using var cts = new CancellationTokenSource(redirectTimeout);
        Task.Run(() =>
        {
            try
            {
                keyInstance.RedirectActivationToAsync(activationArgs).AsTask(cts.Token).GetAwaiter().GetResult();
            }
            catch (OperationCanceledException)
            {
                Log.Error($"Failed to activate existing instance; timed out after {redirectTimeout}.");
            }
            catch (Exception ex)
            {
                Log.Error("Failed to activate existing instance", ex);
            }
            finally
            {
                redirectComplete.Set();
            }
        });
        var handle = new HANDLE(redirectComplete.SafeWaitHandle.DangerousGetHandle());
        PInvoke.CoWaitForMultipleObjects((uint)CWMO_FLAGS.CWMO_DEFAULT, PInvoke.INFINITE, [handle], out _);
    }

    private static async void OnRedirectActivated(object? sender, AppActivationArguments args)
    {
        var dispatcher = await _uiDispatcherReady.Task;
        Current.ShowMainWindow();
        dispatcher.TryEnqueue(() =>
        {
            HandleActivation(args);
        });
    }
}
