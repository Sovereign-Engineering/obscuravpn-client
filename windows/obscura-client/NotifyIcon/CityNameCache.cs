using System;
using System.Collections.Generic;
using System.Threading;
using System.Threading.Tasks;
using log4net;

namespace Obscura_Client.NotifyIcon;

/// <summary>
/// Long-polls the Rust service's `getExitList` ManagerCmd and maintains an in-memory
/// (countryCode, cityCode) → cityName lookup.
/// </summary>
public sealed partial class CityNameCache
{
    static readonly ILog Log = LogManager.GetLogger(typeof(CityNameCache));

    readonly CancellationTokenSource _cts = new();
    readonly Lock _lock = new();
    Dictionary<(string country, string city), string> _names = [];
    Task? _loop;

    public void Start()
    {
        if (_loop != null) throw new InvalidOperationException("already started");
        _loop = Task.Run(() => RunLoop(_cts.Token));
    }

    /// <summary>
    /// Returns a display name for the given (countryCode, cityCode), falling back to the
    /// raw cityCode if the cache is empty or doesn't contain that entry.
    /// </summary>
    public string DisplayName(string countryCode, string cityCode)
    {
        lock (_lock)
        {
            return _names.TryGetValue((countryCode, cityCode), out var name) ? name : cityCode;
        }
    }

    /// <summary>Returns true if the given pin's city is known to the current cache, OR the cache is empty.</summary>
    public bool ContainsOrEmpty(string countryCode, string cityCode)
    {
        lock (_lock)
        {
            if (_names.Count == 0) return true;
            return _names.ContainsKey((countryCode, cityCode));
        }
    }

    async Task RunLoop(CancellationToken ct)
    {
        string? knownVersion = null;
        while (!ct.IsCancellationRequested)
        {
            try
            {
                var cached = await IPCCommand.GetExitList(knownVersion, ct);
                knownVersion = cached.Version;
                var exits = cached.Value?.Exits ?? [];

                var fresh = new Dictionary<(string, string), string>(exits.Length);
                foreach (var e in exits)
                {
                    fresh[(e.CountryCode, e.CityCode)] = e.CityName;
                }
                lock (_lock) { _names = fresh; }
            }
            catch (OperationCanceledException) when (ct.IsCancellationRequested)
            {
                return;
            }
            catch (Exception ex)
            {
                Log.Warn($"getExitList long-poll failed, retrying: {ex.Message}");
                try { await Task.Delay(TimeSpan.FromSeconds(5), ct); }
                catch (OperationCanceledException) { return; }
            }
        }
    }
}
