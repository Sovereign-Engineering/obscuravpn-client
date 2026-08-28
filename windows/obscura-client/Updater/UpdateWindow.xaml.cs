using System;
using System.IO;
using log4net;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using NetSparkleUpdater;
using NetSparkleUpdater.Enums;
using NetSparkleUpdater.Events;
using Obscura_Client.Markdown;
using Windows.ApplicationModel.Core;
using Windows.Foundation;
using Windows.Graphics;
using Windows.Win32;
using Windows.Win32.Foundation;
using Windows.Win32.UI.WindowsAndMessaging;
using WinUIEx;

namespace Obscura_Client.Updater;

/// <summary>
/// Confirm/progress window for installing an available update. Downloads the
/// MSI via NetSparkle and hands off to the installer; the app exits through
/// App.ExitForUpdate once the installer script has been started.
/// </summary>
public sealed partial class UpdateWindow : Window
{
    static readonly ILog Log = LogManager.GetLogger(typeof(UpdateWindow));

    // Fixed content width; the height follows content (notes capped by NotesScroll.MaxHeight in XAML).
    const double WindowWidth = 600;

    readonly SparkleUpdater _sparkle;
    readonly AppCastItem _item;
    bool _downloading;

    public UpdateWindow(AppUpdater updater, AppCastItem item, string releaseNotes)
    {
        _sparkle = updater.Sparkle ?? throw new InvalidOperationException("Updater not configured");
        _item = item;
        InitializeComponent();

        Title = "Obscura VPN Update";
        // Use the modern TitleBar control as the custom title bar, matching MainWindow.
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(AppTitleBar);
        AppWindow.SetIcon(Path.Combine(AppContext.BaseDirectory, "Assets/Icon.ico"));
        if (AppWindow.Presenter is OverlappedPresenter presenter)
        {
            presenter.IsMaximizable = false;
            presenter.IsMinimizable = false;
            presenter.IsResizable = false;
        }

        HeadingText.Text = $"Obscura VPN v{_item.Version} is available";
        CurrentVersionText.Text = $"You have {OsStatus.Instance.SrcVersion}";
        var notes = string.IsNullOrWhiteSpace(releaseNotes)
            ? (string.IsNullOrWhiteSpace(_item.ReleaseNotesLink) ? "" : $"[Release notes]({_item.ReleaseNotesLink})")
            : releaseNotes;
        MarkdownText.Render(ReleaseNotes, notes);
        NotesBorder.Visibility = string.IsNullOrWhiteSpace(notes) ? Visibility.Collapsed : Visibility.Visible;

        // Best-effort pre-show sizing so the first presented frame is already close to its
        // final size, then re-fit once the tree is loaded (fonts/theme fully resolved).
        SizeToContent();
        if (Content is FrameworkElement root)
        {
            root.Loaded += (_, _) => SizeToContent();
        }

        _sparkle.DownloadStarted += OnDownloadStarted;
        _sparkle.DownloadMadeProgress += OnDownloadMadeProgress;
        _sparkle.DownloadFinished += OnDownloadFinished;
        _sparkle.DownloadHadError += OnDownloadHadError;
        _sparkle.InstallUpdateFailed += OnInstallUpdateFailed;
        Closed += OnClosed;
    }

    internal void BringToFront()
    {
        var hwnd = (HWND)WinRT.Interop.WindowNative.GetWindowHandle(this);
        PInvoke.ShowWindow(hwnd, SHOW_WINDOW_CMD.SW_NORMAL);
        Activate();
        PInvoke.SetForegroundWindow(hwnd);
    }

    /// <summary>
    /// Fits the window to its content at a fixed width. The notes ScrollViewer caps its own
    /// height (MaxHeight in XAML), so the measured height stays bounded.
    /// </summary>
    void SizeToContent()
    {
        if (Content is not FrameworkElement root)
        {
            return;
        }
        try
        {
            // Before the window is shown the XamlRoot doesn't exist yet; derive the scale
            // from the window's DPI instead so the pre-show pass can still size it.
            var scale = root.XamlRoot?.RasterizationScale ?? this.GetDpiForWindow() / 96.0;
            root.Measure(new Size(WindowWidth, double.PositiveInfinity));
            var height = root.DesiredSize.Height;
            if (height <= 0)
            {
                return;
            }
            // Subtract no clipping from the root desired height
            AppWindow.ResizeClient(new SizeInt32(
                (int)Math.Ceiling(WindowWidth * scale),
                (int)Math.Ceiling((height - AppTitleBar.DesiredSize.Height) * scale)));
        }
        catch (Exception ex)
        {
            Log.Warn($"failed to size update window to content: {ex.Message}");
        }
    }

    async void OnInstallClick(object sender, RoutedEventArgs e)
    {
        InstallButton.IsEnabled = false;
        ErrorText.Visibility = Visibility.Collapsed;
        ShowProgress("Downloading update...", indeterminate: true);
        _downloading = true;
        try
        {
            await _sparkle.InitAndBeginDownload(_item);
        }
        catch (Exception ex)
        {
            Log.Error($"update download failed to start: {ex}");
            ShowError($"Could not download the update: {ex.Message}");
        }
    }

    void OnLaterClick(object sender, RoutedEventArgs e)
    {
        // Deferred: closing synchronously from inside the button's input event tears the
        // visual tree down while the event is still routing, which can crash XAML (0xC000027B).
        DispatcherQueue.TryEnqueue(Close);
    }

    void OnDownloadStarted(AppCastItem item, string path)
    {
        DispatcherQueue.TryEnqueue(() => ShowProgress("Downloading update...", indeterminate: false));
    }

    void OnDownloadMadeProgress(object sender, AppCastItem item, ItemDownloadProgressEventArgs args)
    {
        DispatcherQueue.TryEnqueue(() => DownloadProgress.Value = args.ProgressPercentage);
    }

    void OnDownloadFinished(AppCastItem item, string path)
    {
        DispatcherQueue.TryEnqueue(async () =>
        {
            _downloading = false;
            ShowProgress("Installing update...", indeterminate: true);
            try
            {
                // Verifies the signature again, starts the installer script, and
                // exits the app via the CloseApplication handler.
                await _sparkle.InstallUpdate(item, path);
            }
            catch (Exception ex)
            {
                Log.Error($"update install failed: {ex}");
                ShowError($"Could not install the update: {ex.Message}");
            }
        });
    }

    void OnDownloadHadError(AppCastItem item, string? path, Exception exception)
    {
        Log.Error($"update download failed: {exception}");
        DispatcherQueue.TryEnqueue(() =>
        {
            _downloading = false;
            ShowError($"Could not download the update: {exception.Message}");
        });
    }

    bool OnInstallUpdateFailed(InstallUpdateFailureReason reason, string? installPath)
    {
        Log.Error($"update install failed: {reason} ({installPath})");
        DispatcherQueue.TryEnqueue(() => ShowError($"Could not install the update ({reason})."));
        return true;
    }

    void ShowProgress(string message, bool indeterminate)
    {
        ProgressPanel.Visibility = Visibility.Visible;
        DownloadProgress.IsIndeterminate = indeterminate;
        ProgressText.Text = message;
        SizeToContent();
    }

    void ShowError(string message)
    {
        ProgressPanel.Visibility = Visibility.Collapsed;
        ErrorText.Text = message;
        ErrorText.Visibility = Visibility.Visible;
        InstallButton.IsEnabled = true;
        SizeToContent();
    }

    void OnClosed(object sender, WindowEventArgs args)
    {
        // Known WinUI issue: destroying a window with an active SystemBackdrop can raise a
        // stowed exception (0xC000027B) during teardown. Detach the backdrop first.
        SystemBackdrop = null;
        if (_downloading)
        {
            _sparkle.CancelFileDownload();
        }
        _sparkle.DownloadStarted -= OnDownloadStarted;
        _sparkle.DownloadMadeProgress -= OnDownloadMadeProgress;
        _sparkle.DownloadFinished -= OnDownloadFinished;
        _sparkle.DownloadHadError -= OnDownloadHadError;
        _sparkle.InstallUpdateFailed -= OnInstallUpdateFailed;
    }
}
