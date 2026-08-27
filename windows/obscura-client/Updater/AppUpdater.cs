using System;
using System.Globalization;
using System.Linq;
using System.Net.Http;
using System.Reflection;
using System.Threading;
using System.Threading.Tasks;
using log4net;
using Microsoft.UI.Dispatching;
using Microsoft.Windows.AppNotifications;
using Microsoft.Windows.AppNotifications.Builder;
using NetSparkleUpdater;
using NetSparkleUpdater.Enums;
using NetSparkleUpdater.SignatureVerifiers;

namespace Obscura_Client.Updater;

/// <summary>
/// Owns the NetSparkle updater. Check results and errors are published through
/// OsStatus.Instance.UpdaterStatus. Installs are driven by UpdateWindow.
/// </summary>
public sealed class AppUpdater
{
    static readonly ILog Log = LogManager.GetLogger(typeof(AppUpdater));
    const string Ed25519PublicKeyBase64 = "gV8HXwQNa8vCmv82QbiAkFSpGNFDxWbY21UPQ9e/sWk=";
    const string ManifestUrlBase = "https://windows-pkgs.obscura.com/";
    internal SparkleUpdater Sparkle { get; }
    internal AppCastItem? AvailableUpdate { get; private set; }
    // Markdown release notes fetched for AvailableUpdate; passed to the update window separately
    // from the AppCastItem (which carries no notes of its own).
    internal string AvailableReleaseNotes { get; private set; } = "";
    readonly DispatcherQueue _uiDispatcher;
    // Only fetches release notes; anything larger than this is not release notes.
    static readonly HttpClient HttpClient = new() { MaxResponseContentBufferSize = 256 * 1024 };
    UpdateWindow? _updateWindow;
    int _checkInProgress;
    // Version we last raised a notification for to avoid redundant notifications
    string? _notifiedVersion;

    public AppUpdater(DispatcherQueue uiDispatcher)
    {
        _uiDispatcher = uiDispatcher;
#if TARGET_AMD64
        const string arch = "x64";
#elif TARGET_ARM64
        const string arch = "arm64";
#else
#error Unsupported architecture
#endif
        var appcastUrl = $"{ManifestUrlBase}update-{arch}.xml";
        Log.Info($"Appcast URL: {appcastUrl}");
        Sparkle = new SparkleUpdater(
            appcastUrl,
            new Ed25519Checker(SecurityMode.Strict, Ed25519PublicKeyBase64))
        {
            LogWriter = new SparkleLogger(),
            // MSI is configured to auto-launch on passive installs
            RelaunchAfterUpdate = false,
            // msiexec: progress bar only, no wizard pages
            CustomInstallerArguments = "/passive",
        };
        // Invoked after the installer script has been started; the script waits
        // for this process to exit before running msiexec.
        Sparkle.CloseApplication += () => App.Current.ExitForUpdate();
    }

    /// <summary>
    /// Check the appcast and publish the outcome on OsStatus. Never throws; all
    /// failures surface as an "error" updater status.
    /// </summary>
    public async Task CheckForUpdatesAsync()
    {
        if (_updateWindow != null)
        {
            Log.Info("skipping update check; install window is open");
            return;
        }
        if (Interlocked.Exchange(ref _checkInProgress, 1) == 1)
        {
            Log.Info("update check already in progress");
            return;
        }
        try
        {
            SetStatus(new SparkleUpdaterStatus { Type = UpdaterStatusType.Initiated });
            var info = await Sparkle.CheckForUpdatesQuietly();
            switch (info.Status)
            {
                case UpdateStatus.UpdateAvailable when info.Updates is { Count: > 0 }:
                    AvailableUpdate = info.Updates[0];
                    Log.Info($"update available: {AvailableUpdate.Version}");
                    AvailableReleaseNotes = await FetchReleaseNotesAsync(AvailableUpdate);
                    SetStatus(new SparkleUpdaterStatus
                    {
                        Type = UpdaterStatusType.Available,
                        Appcast = Summarize(AvailableUpdate, AvailableReleaseNotes),
                    });
                    break;
                case UpdateStatus.UpdateNotAvailable:
                case UpdateStatus.UserSkipped:
                    AvailableUpdate = null;
                    Log.Info("no update available");
                    // errorCode 1 matches Sparkle's "up to date" reason, which the
                    // web UI renders as "you have the latest version"
                    SetStatus(new SparkleUpdaterStatus
                    {
                        Type = UpdaterStatusType.NotFound,
                        ErrorCode = 1,
                    });
                    break;
                default:
                    Log.Warn("update check could not determine update status");
                    SetStatus(new SparkleUpdaterStatus
                    {
                        Type = UpdaterStatusType.Error,
                        Error = "Could not check for updates. Please try again later.",
                    });
                    break;
            }
        }
        catch (Exception ex)
        {
            Log.Error($"update check failed: {ex}");
            SetStatus(new SparkleUpdaterStatus
            {
                Type = UpdaterStatusType.Error,
                Error = ex.Message,
            });
        }
        finally
        {
            Interlocked.Exchange(ref _checkInProgress, 0);
        }
    }

    /// <summary>
    /// Periodic (timer-driven) check: refreshes the appcast and, when a new version is
    /// available, raises a notification for it once. Unlike ShowPromptIfNeededAsync this
    /// shows a notification instead of opening the update window.
    /// </summary>
    public async Task CheckAndNotifyAsync()
    {
        await CheckForUpdatesAsync();
        var update = AvailableUpdate;
        if (update == null || update.Version == _notifiedVersion)
        {
            return;
        }
        _notifiedVersion = update.Version;
        Log.Info($"notifying update available: {update.Version}");
        try
        {
            AppNotificationManager.Default.Show(new AppNotificationBuilder()
                .AddArgument(NotificationActions.ArgumentKey, NotificationActions.ShowUpdate)
                .AddText("A new version of Obscura VPN is available.")
                .AddText($"Version {update.Version} is ready to install.")
                .BuildNotification());
        }
        catch (Exception ex)
        {
            Log.Warn($"failed to show update notification: {ex}");
        }
    }

    static readonly TimeSpan StaleBuildAge = TimeSpan.FromDays(30);

    /// <summary>
    /// The update check run once at startup. Engaged users (logged in and past the new-account
    /// flow) are prompted to install immediately. Everyone else gets a passive notification
    /// unless their build is stale, in which case they are prompted too.
    /// </summary>
    public async Task FirstUpdateCheck()
    {
        bool engaged;
        try
        {
            using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(10));
            var status = await IPCCommand.GetStatus(null, cts.Token);
            engaged = status.AccountId != null && !status.InNewAccountFlow;
        }
        catch (Exception ex)
        {
            Log.Warn($"first update check: could not get NEStatus to check for login status: {ex.Message}; will prompt if outdated");
            await CheckAndPromptAsync();
            return;
        }

        if (engaged)
        {
            Log.Info($"first update check: user is engaged; will prompt if outdated");
            await CheckAndPromptAsync();
            return;
        }

        var buildDate = GetBuildDate();
        if (buildDate != default && DateTime.UtcNow - buildDate >= StaleBuildAge)
        {
            Log.Info("first update check: build is stale; will prompt if outdated");
            await CheckAndPromptAsync();
        }
        else
        {
            Log.Info("first update check: build is not stale; will notify if outdated");
            await CheckAndNotifyAsync();
        }
    }

    public async Task CheckAndPromptAsync()
    {
        await CheckForUpdatesAsync();
        if (AvailableUpdate != null)
        {
            PromptInstall();
        }
    }

    public void PromptInstall()
    {
        var item = AvailableUpdate ?? throw new InvalidOperationException("No update available to install");
        _uiDispatcher.TryEnqueue(() =>
        {
            if (_updateWindow == null)
            {
                _updateWindow = new UpdateWindow(this, item, AvailableReleaseNotes);
                _updateWindow.Closed += (_, _) => _updateWindow = null;
            }
            _updateWindow.BringToFront();
        });
    }

    /// <summary>
    /// The appcast carries release notes as a URL (ReleaseNotesLink) rather than inline,
    /// so fetch the linked document. Falls back to any inline Description on failure.
    /// </summary>
    static async Task<string> FetchReleaseNotesAsync(AppCastItem item)
    {
        var link = item.ReleaseNotesLink;
        if (string.IsNullOrWhiteSpace(link))
        {
            return item.Description ?? "";
        }
        try
        {
            using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(10));
            return await HttpClient.GetStringAsync(link, cts.Token);
        }
        catch (Exception ex)
        {
            Log.Warn($"failed to fetch release notes from {link}: {ex.Message}");
            return item.Description ?? "";
        }
    }

    // Raised after each completed check; true when an update is known to be available.
    internal event Action<bool>? UpdateAvailabilityChanged;

    void SetStatus(SparkleUpdaterStatus status)
    {
        OsStatus.Instance.Update(s => s.UpdaterStatus = status);
        if (status.Type != UpdaterStatusType.Initiated)
        {
            UpdateAvailabilityChanged?.Invoke(AvailableUpdate != null);
        }
    }

    static AppcastSummary Summarize(AppCastItem item, string releaseNotes) => new()
    {
        Date = item.PublicationDate == default ? "" : item.PublicationDate.ToString("R"),
        Description = releaseNotes,
        Version = item.Version ?? "",
        MinSystemVersionOk = true,
    };

    class SparkleLogger : NetSparkleUpdater.Interfaces.ILogger
    {
        static readonly ILog Log = LogManager.GetLogger("NetSparkle");

        public void PrintMessage(string message, params object[]? arguments)
        {
            try
            {
                Log.Info(arguments is { Length: > 0 } ? string.Format(message, arguments) : message);
            }
            catch (FormatException)
            {
                Log.Info(message);
            }
        }
    }

    public static DateTime GetBuildDate()
    {
        // Embedded via the AssemblyMetadata item in the csproj
        var buildDate = Assembly.GetExecutingAssembly()
            .GetCustomAttributes<AssemblyMetadataAttribute>()
            .FirstOrDefault(a => a.Key == "BuildDate")?.Value;

        if (buildDate == null)
        {
            Log.Warn("build date unknown: no BuildDate assembly metadata");
            return default;
        }
        if (!DateTime.TryParseExact(buildDate, "yyyyMMddHHmmss", CultureInfo.InvariantCulture, DateTimeStyles.None, out var result))
        {
            Log.Warn($"build date unknown: could not parse '{buildDate}'");
            return default;
        }

        Log.Info($"build date: {result}");
        return result;
    }
}
