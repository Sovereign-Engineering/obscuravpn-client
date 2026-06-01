using System;
using System.Threading;
using System.Threading.Tasks;
using log4net;

namespace Obscura_Client.NotifyIcon;

/// <summary>
/// Long-polls the Rust service's `getStatus` ManagerCmd, raising <see cref="StatusChanged"/>
/// whenever the version bumps.
/// </summary>
public sealed partial class StatusSubscriber
{
    static readonly ILog Log = LogManager.GetLogger(typeof(StatusSubscriber));

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
                var status = await IPCCommand.GetStatus(knownVersion, ct);
                knownVersion = status.Version;
                Current = status;
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
