using System;
using System.IO;
using log4net;
using log4net.Appender;
using log4net.Config;
using log4net.Layout;
using Microsoft.UI.Xaml;

// To learn more about WinUI, the WinUI project structure,
// and more about our project templates, see: http://aka.ms/winui-project-info.

namespace Obscura_Client;

/// <summary>
/// Provides application-specific behavior to supplement the default Application class.
/// </summary>
public partial class App : Application
{
    private static readonly ILog Log = LogManager.GetLogger(typeof(App));
    private Window? _window;

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

    protected override void OnLaunched(Microsoft.UI.Xaml.LaunchActivatedEventArgs args)
    {
        _window = new MainWindow();
        _window.Activate();
    }
}
