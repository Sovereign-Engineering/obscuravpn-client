using System;
using System.Net.NetworkInformation;
using System.Text.Json;
using System.Threading.Tasks;
using log4net;

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
    public bool InProgress { get; set; } = false;
    public string? LatestPath { get; set; } = null;
    public int InProgressCounter { get; set; } = 0;
}

public class LoginItemStatus
{
    public bool Registered { get; set; } = false;
    public string? Error { get; set; } = null;
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

    private readonly object _lock = new();
    private TaskCompletionSource _versionChanged = new(TaskCreationOptions.RunContinuationsAsynchronously);

    public string Version { get; private set; } = Guid.NewGuid().ToString();
    public NavigationView NavigationView { get; private set; } = NavigationView.Connection;
    public bool InternetAvailable { get; private set; } = false;
    public string SrcVersion { get; } = GetSrcVersion();

    private static string GetSrcVersion()
    {
        var version = System.Reflection.Assembly.GetExecutingAssembly().GetName().Version?.ToString(2) ?? "unknown";
#if DEBUG
        return $"{version}-dev";
#else
        return version;
#endif
    }
    public DebugBundleStatus DebugBundleStatus { get; set; } = new();
    public LoginItemStatus? LoginItemStatus { get; set; } = null;
    public SparkleUpdaterStatus UpdaterStatus { get; set; } = new();
    public bool CanSendMail { get; } = true;

    private OsStatus()
    {
        InternetAvailable = NetworkInterface.GetIsNetworkAvailable();
        Log.Info($"initial internet availability: {InternetAvailable}");
        NetworkChange.NetworkAvailabilityChanged += (_, e) =>
        {
            Log.Info($"internet availability changed: {e.IsAvailable}");
            Update(s => s.InternetAvailable = e.IsAvailable);
        };
    }

    /// <summary>
    /// Update a field and bump the version, notifying any waiters.
    /// </summary>
    public void Update(Action<OsStatus> mutate)
    {
        lock (_lock)
        {
            mutate(this);
            Version = Guid.NewGuid().ToString();
            var old = _versionChanged;
            _versionChanged = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
            old.TrySetResult();
        }
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
