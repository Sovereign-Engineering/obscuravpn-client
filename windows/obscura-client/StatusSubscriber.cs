using System;
using System.Threading;
using System.Threading.Tasks;
using log4net;

namespace Obscura_Client;

/// <summary>
/// Long-polls the Rust service's `getStatus` ManagerCmd, raising <see cref="StatusChanged"/>
/// whenever the version bumps.
/// </summary>
public sealed partial class StatusSubscriber
{
    static readonly ILog Log = LogManager.GetLogger(typeof(StatusSubscriber));
    public static StatusSubscriber Instance { get; } = new();
    private StatusSubscriber() { }

    static readonly TimeSpan PollTimeout = TimeSpan.FromSeconds(5);

    readonly CancellationTokenSource _cts = new();
    Task? _loop;

    public NeStatus? Current { get; private set; }
    public event Action<NeStatus>? StatusChanged;

    public void Start()
    {
        if (_loop != null) throw new InvalidOperationException("already started");
        _loop = Task.Run(() => RunLoop(_cts.Token));
    }

    async Task RunLoop(CancellationToken ct)
    {
        string? knownVersion = null;
        while (!ct.IsCancellationRequested)
        {
            try
            {
                NeStatus status;
                using (var watchdog = CancellationTokenSource.CreateLinkedTokenSource(ct))
                {
                    if (knownVersion == null)
                    {
                        watchdog.CancelAfter(PollTimeout);
                    }
                    try
                    {
                        status = await IPCCommand.GetStatus(knownVersion, watchdog.Token);
                    }
                    catch (OperationCanceledException) when (knownVersion != null && !ct.IsCancellationRequested)
                    {
                        // Retry with knownVersion null to confirm service degradation
                        knownVersion = null;
                        continue;
                    }
                }
                knownVersion = status.Version;
                Current = status;
                OsStatus.Instance.ReportServiceHealthy(status);
                try { StatusChanged?.Invoke(status); }
                catch (Exception ex) { Log.Error($"StatusChanged handler threw: {ex}"); }
            }
            catch (OperationCanceledException) when (ct.IsCancellationRequested)
            {
                return;
            }
            catch (Exception ex)
            {
                Log.Warn($"getStatus long-poll failed, retrying: {ex.Message}");
                knownVersion = null;
                OsStatus.Instance.ReportServiceDegraded(ObscuraService.Diagnose());
                await DelayBeforeRetry(ct);
            }
        }
    }

    static async Task DelayBeforeRetry(CancellationToken ct)
    {
        try { await Task.Delay(TimeSpan.FromSeconds(1), ct); }
        catch (OperationCanceledException) { }
    }
}
