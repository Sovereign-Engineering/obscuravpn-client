using System;
using System.Diagnostics;
using System.Linq;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Threading;
using System.Threading.Tasks;
using log4net;
using Windows.ApplicationModel;
using Windows.Networking.Connectivity;

namespace Obscura_Client;

public enum NavigationView
{
    Developer,
    Connection,
    Location,
    Account,
    Help,
    About,
    Settings,
}

public class DebugBundleStatus
{
    public int InProgressCounter { get; set; } = 0;
    public bool InProgress => InProgressCounter > 0;
    public string? LatestPath { get; set; } = null;

    public void Start() => InProgressCounter += 1;
    public void Finish() => InProgressCounter -= 1;
    public void SetPath(string path) => LatestPath = path;
    public void MarkError() => LatestPath = null;
}

public class LoginItemStatus
{
    public bool Registered { get; set; } = false;
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Error { get; set; } = null;
}

public static class LoginItem
{
    private static readonly ILog Log = LogManager.GetLogger(typeof(LoginItem));
    private const string TaskId = "ObscuraVPNStartup";

    static bool IsRegistered(StartupTaskState startupTask)
    {
        return startupTask == StartupTaskState.Enabled || startupTask == StartupTaskState.EnabledByPolicy;
    }

    public static async Task RefreshStatusAsync()
    {
        try
        {
            var task = await StartupTask.GetAsync(TaskId);
            OsStatus.Instance.Update(s => s.LoginItemStatus = new LoginItemStatus { Registered = IsRegistered(task.State) });
        }
        catch (Exception ex)
        {
            Log.Error($"failed to get login item status: {ex}");
            OsStatus.Instance.Update(s => s.LoginItemStatus = new LoginItemStatus { Error = ex.Message });
        }
    }

    public static async Task RegisterAsync()
    {
        try
        {
            var task = await StartupTask.GetAsync(TaskId);
            var newState = await task.RequestEnableAsync();
            OsStatus.Instance.Update(s => s.LoginItemStatus = new LoginItemStatus { Registered = IsRegistered(newState) });
            if (task.State == StartupTaskState.DisabledByUser)
            {
                // The user disabled startup themselves,
                // so we take them straight to the Startup settings page to re-enable it.
                // Delay opening startup settings so the error message can surface first;
                _ = Task.Run(async () =>
                {
                    await Task.Delay(TimeSpan.FromSeconds(2));
                    await OpenStartupSettingsAsync();
                });
                throw new Exception("\"Obscura VPN\" Startup was disabled from Windows settings.");
            }
        }
        catch (Exception ex)
        {
            Log.Error($"failed to register app at login: {ex}");
            SetError(ex.Message);
            throw;
        }
    }

    public static async Task UnregisterAsync()
    {
        try
        {
            var task = await StartupTask.GetAsync(TaskId);
            task.Disable();
            OsStatus.Instance.Update(s => s.LoginItemStatus = new LoginItemStatus { Registered = IsRegistered(task.State) });
        }
        catch (Exception ex)
        {
            Log.Error($"failed to unregister app at login: {ex}");
            SetError(ex.Message);
            throw;
        }
    }

    private static async Task OpenStartupSettingsAsync()
    {
        try
        {
            Process.Start(new ProcessStartInfo
            {
                FileName = "ms-settings:startupapps",
                UseShellExecute = true,
            });
        }
        catch (Exception ex)
        {
            Log.Error($"failed to open startup settings: {ex}");
        }
    }

    private static void SetError(string error)
    {
        OsStatus.Instance.Update(s =>
        {
            if (s.LoginItemStatus is not null) s.LoginItemStatus.Error = error;
            else s.LoginItemStatus = new LoginItemStatus { Error = error };
        });
    }
}

public class SparkleUpdaterStatus
{
    public string Type { get; set; } = "uninitiated";
    public object? Appcast { get; set; } = null;
    public string? Error { get; set; } = null;
    public long? ErrorCode { get; set; } = null;
}

public class OsStatus
{
    private static readonly ILog Log = LogManager.GetLogger(typeof(OsStatus));
    public static OsStatus Instance { get; } = new OsStatus();

    private readonly Lock _lock = new();
    private TaskCompletionSource _versionChanged = new(TaskCreationOptions.RunContinuationsAsynchronously);

    public string Version { get; private set; } = Guid.NewGuid().ToString();
    public NavigationView NavigationView { get; private set; } = NavigationView.Connection;
    public bool InternetAvailable { get; private set; } = false;
    public string SrcVersion { get; } = GetSrcVersion();

    public static string GetSrcVersion()
    {
        var version = System.Reflection.Assembly.GetExecutingAssembly().GetName().Version?.ToString(2) ?? "unknown";
#if DEBUG
        return $"v{version}-dev";
#else
        return $"v{version}";
#endif
    }
    public DebugBundleStatus DebugBundleStatus { get; set; } = new();
    public LoginItemStatus? LoginItemStatus { get; set; } = null;
    public SparkleUpdaterStatus UpdaterStatus { get; set; } = new();
    public bool CanSendMail { get; } = true;
    public ServiceStatus ServiceStatus { get; private set; } = ServiceStatus.Initializing;

    private string? _healthyStatusVersion;
    private WindowsServiceDegradation? _serviceDegradation;
    private NeStatus? _lastHealthyStatus;

    private OsStatus()
    {
        // On first run, getting internet profiles on the default thread can cause issues on Windows 10
        NetworkInformation.NetworkStatusChanged += OnNetworkStatusChanged;
        Task.Run(() =>
        {
            try
            {
                var available = GetInternetAvailable();
                Log.Info($"initial internet availability: {available}");
                Update(s => s.InternetAvailable = available);
            }
            catch (Exception ex)
            {
                Log.Error($"failed to initialize internet availability: {ex}");
            }
        });
    }

    // Fires on an MTA worker thread, outside any XAML handler: an exception escaping here
    // takes down the process.
    static void OnNetworkStatusChanged(object? sender)
    {
        try
        {
            var available = GetInternetAvailable();
            Log.Info($"internet availability changed: {available}");
            Instance.Update(s => s.InternetAvailable = available);
        }
        catch (Exception ex)
        {
            Log.Warn($"network status update failed: {ex.Message}");
        }
    }

    private static bool GetInternetAvailable()
    {
        var profile = NetworkInformation.GetInternetConnectionProfile();
        if (profile != null)
        {
            var level = profile.GetNetworkConnectivityLevel();
            Log.Info($"internet connectivity level: {level}");
            return level == NetworkConnectivityLevel.InternetAccess;
        }
        Log.Info($"GetInternetConnectionProfile() returned null");
        var profiles = NetworkInformation.GetConnectionProfiles()
            .Select(p => (p.ProfileName, AdapterId: p.NetworkAdapter?.NetworkAdapterId, Level: p.GetNetworkConnectivityLevel()))
            .Where(p => p.Level != NetworkConnectivityLevel.None)
            .ToList();
        foreach (var (name, adapterId, level) in profiles)
        {
            Log.Info($"profile '{name}' adapter={adapterId} level={level}");
        }
        return profiles.Any(p => p.Level == NetworkConnectivityLevel.InternetAccess);
    }

    /// <summary>
    /// Update a field and bump the version, notifying any waiters.
    /// </summary>
    public void Update(Action<OsStatus> mutate)
    {
        UpdateIf(s =>
        {
            mutate(s);
            return true;
        });
    }

    public event Action<OsStatus>? Changed;

    /// <summary>
    /// Update that only bumps the version (and notifies waiters) when <paramref name="mutate"/> returns true.
    /// </summary>
    public void UpdateIf(Func<OsStatus, bool> mutate)
    {
        lock (_lock)
        {
            if (!mutate(this)) return;
            Version = Guid.NewGuid().ToString();
            var old = _versionChanged;
            _versionChanged = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
            old.TrySetResult();
        }
        try { Changed?.Invoke(this); }
        catch (Exception ex) { Log.Error($"Changed handler threw: {ex}"); }
    }

    public void ReportServiceHealthy(NeStatus status)
    {
        UpdateIf(s =>
        {
            if (s._healthyStatusVersion == status.Version) return false;
            if (s._serviceDegradation != null) Log.Info($"service recovered from degradation: {s._serviceDegradation}");
            s._healthyStatusVersion = status.Version;
            s._serviceDegradation = null;
            s._lastHealthyStatus = status;
            s.ServiceStatus = new ServiceStatus { Healthy = status };
            return true;
        });
    }

    public void ReportServiceDegraded(WindowsServiceDegradation degradation)
    {
        UpdateIf(s =>
        {
            if (s._serviceDegradation == degradation) return false;
            Log.Warn($"service degraded: {degradation}");
            s._serviceDegradation = degradation;
            s._healthyStatusVersion = null;
            s.ServiceStatus = new ServiceStatus
            {
                Degraded = new DegradedServiceInfo
                {
                    LastStatus = s._lastHealthyStatus,
                    WindowsDegradation = degradation,
                },
            };
            return true;
        });
    }

    /// <summary>
    /// Set the navigation view and bump the version.
    /// </summary>
    public void SetNavigationView(NavigationView view)
    {
        Update(s => s.NavigationView = view);
    }

    /// <summary>
    /// Returns the current OsStatus as JSON if the version differs from knownVersion,
    /// otherwise waits until the version changes before returning.
    /// </summary>
    public async Task<string> GetJsonWhenChanged(string? knownVersion)
    {
        while (true)
        {
            Task waitTask;
            lock (_lock)
            {
                if (Version != knownVersion)
                {
                    return ToJson();
                }
                waitTask = _versionChanged.Task;
            }
            await waitTask;
        }
    }

    public string ToJson()
    {
        lock (_lock)
        {
            return JsonSerializer.Serialize(this, JsonConfig.Options);
        }
    }
}
