using System;
using System.IO;
using System.Threading;
using log4net;
using log4net.Appender;
using log4net.Config;
using log4net.Layout;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Obscura_Client.NotifyIcon;

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

    /// <summary>
    /// Initializes the singleton application object.  This is the first line of authored code
    /// executed, and as such is the logical equivalent of main() or WinMain().
    /// </summary>
    public App()
    {
        ConfigureLogging();
        Log.Info("logging initialized");
        InitializeComponent();
#if DEBUG
        DevServer.Start();
        AppDomain.CurrentDomain.ProcessExit += (s, e) => DevServer.Stop();
        UnhandledException += (s, e) => DevServer.Stop();
#endif
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

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        _notifyIcon = new NotifyIconManager(this, DispatcherQueue.GetForCurrentThread());
        _window = new MainWindow();
        _window.Activate();
    }

    internal void ShowMainWindow()
    {
        if (_window == null)
        {
            Log.Warn("ShowMainWindow was called, but window has to be recreated");
            _window = new MainWindow();
            _window.Activate();
            return;
        }
        _window.ShowAndActivate();
    }

    /// <summary>
    /// User-initiated quit path from the tray menu. Disconnects the tunnel, disposes the
    /// tray, then terminates the process. Errors during disconnect are logged but do not
    /// block quitting — getting the user out is more important than a clean disconnect.
    /// </summary>
    internal async void RequestQuit()
    {
        _window?.Close();
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
}
