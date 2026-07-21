using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Threading;
using System.Threading.Tasks;
using log4net;
using Microsoft.UI.Xaml;
using Microsoft.Windows.AppNotifications;
using Microsoft.Windows.AppNotifications.Builder;

namespace Obscura_Client;

public class InvokeCommand
{
    private static readonly ILog Log = LogManager.GetLogger(typeof(InvokeCommand));

    public GetOsStatusCommand? GetOsStatus { get; set; }
    public SetNavigationViewCommand? SetNavigationView { get; set; }
    public SetColorSchemeCommand? SetColorScheme { get; set; }
    public IPCCommand? JsonFfiCmd { get; set; }
    public StartTunnelCommand? StartTunnel { get; set; }
    public StopTunnelCommand? StopTunnel { get; set; }
    public RevealItemInDirCommand? RevealItemInDir { get; set; }
    public DebugBundleCommand? DebugBundle { get; set; }
    public SendNotificationCommand? SendNotification { get; set; }

    public static IObscuraCommand Parse(string commandJson)
    {
        InvokeCommand? invoke;
        try
        {
            invoke = JsonSerializer.Deserialize<InvokeCommand>(commandJson, JsonConfig.Options);
        }
        catch (JsonException ex)
        {
            Log.Error($"Failed to parse command JSON: {ex.Message}: {commandJson}");
            throw new ArgumentException("Failed to parse command", ex);
        }
        if (invoke == null) throw new ArgumentException("Failed to parse command");

        if (invoke.GetOsStatus != null) return invoke.GetOsStatus;
        if (invoke.SetNavigationView != null) return invoke.SetNavigationView;
        if (invoke.SetColorScheme != null) return invoke.SetColorScheme;
        if (invoke.JsonFfiCmd != null) return invoke.JsonFfiCmd;
        if (invoke.StartTunnel != null) return invoke.StartTunnel;
        if (invoke.StopTunnel != null) return invoke.StopTunnel;
        if (invoke.RevealItemInDir != null) return invoke.RevealItemInDir;
        if (invoke.DebugBundle != null) return invoke.DebugBundle;
        if (invoke.SendNotification != null) return invoke.SendNotification;
        Log.Warn($"Unknown command: {commandJson}");
        throw new NotSupportedException($"Unknown command: {commandJson}");
    }
}

public interface IObscuraCommand
{
    static readonly Task<string> UnitResponse = Task.FromResult("null");

    Task<string> RunAsync();
}

public class GetOsStatusCommand : IObscuraCommand
{
    public string? KnownVersion { get; set; }

    public async Task<string> RunAsync()
    {
        return await OsStatus.Instance.GetJsonWhenChanged(KnownVersion);
    }
}

public class SetNavigationViewCommand : IObscuraCommand
{
    public required NavigationView View { get; set; }

    public Task<string> RunAsync()
    {
        App.Current.SelectNavigationView(View);
        return IObscuraCommand.UnitResponse;
    }
}

public enum AppColorScheme
{
    Dark,
    Light,
    Auto,
}

public class SetColorSchemeCommand : IObscuraCommand
{
    public required AppColorScheme Value { get; set; }

    public Task<string> RunAsync()
    {
        var theme = Value switch
        {
            AppColorScheme.Dark => ElementTheme.Dark,
            AppColorScheme.Light => ElementTheme.Light,
            _ => ElementTheme.Default,
        };
        App.Current.ApplyColorScheme(theme);
        ClientSettings.ColorScheme = theme;
        return Task.FromResult("null");
    }
}

public class ManagerError : Exception
{
    public string ErrorJson { get; }

    public ManagerError(string errorJson) : base(errorJson)
    {
        ErrorJson = errorJson;
    }
}
public interface IIPCCommandArg
{
    public string CommandName();
}

public class IPCCommand : IObscuraCommand
{
    private static readonly ILog Log = LogManager.GetLogger(typeof(IPCCommand));
    public required string Cmd { get; set; }
    public double? TimeoutMs { get; set; }

    public static Task<string> RunWithArgAsync(IIPCCommandArg command, CancellationToken ct = default)
    {
        var wrapped = new Dictionary<string, object?> { [command.CommandName()] = command };
        var json = JsonSerializer.Serialize(wrapped, JsonConfig.Options);
        return SendAndParseAsync(json, ct);
    }

    public static async Task<NeStatus> GetStatus(string? knownVersion, CancellationToken ct)
    {
        var ok = await RunWithArgAsync(new GetStatusArgs { KnownVersion = knownVersion }, ct);
        return JsonSerializer.Deserialize<NeStatus>(ok, JsonConfig.Options)
            ?? throw new InvalidOperationException($"getStatus returned null body: {ok}");
    }

    public static async Task<ExitListEnvelope> GetExitList(string? knownVersion, CancellationToken ct)
    {
        var ok = await RunWithArgAsync(new GetExitListArgs { KnownVersion = knownVersion }, ct);
        return JsonSerializer.Deserialize<ExitListEnvelope>(ok, JsonConfig.Options)
            ?? throw new InvalidOperationException($"getExitList returned null body: {ok}");
    }

    public async Task<string> RunAsync()
    {
        if (string.IsNullOrEmpty(Cmd))
            throw new ArgumentException("jsonFfiCmd missing required `cmd` string property");

        using var cts = TimeoutMs is > 0 ? new CancellationTokenSource(TimeSpan.FromMilliseconds(TimeoutMs.Value)) : null;
        var cancellationToken = cts?.Token ?? default;

        return await SendAndParseAsync(Cmd, cancellationToken);
    }

    static async Task<string> SendAndParseAsync(string commandJson, CancellationToken ct)
    {
        var response = await ServiceIpc.SendCommand(commandJson, ct);

        IPCResponse? result;
        try
        {
            result = JsonSerializer.Deserialize<IPCResponse>(response, JsonConfig.Options);
        }
        catch (JsonException ex)
        {
            throw new ArgumentException($"Failed to parse IPC response: {response}", ex);
        }
        if (result == null) throw new ArgumentException($"Unexpected IPC response format: {response}");

        if (result.Ok.ValueKind != JsonValueKind.Undefined) return result.Ok.GetRawText();

        if (result.Err.ValueKind != JsonValueKind.Undefined)
        {
            string errString;
            try
            {
                errString = result.Err.Deserialize<string>(JsonConfig.Options)
                    ?? result.Err.GetRawText();
            }
            catch (JsonException ex)
            {
                Log.Error($"Failed to deserialize error as string, falling back to raw text: {ex.Message}");
                errString = result.Err.GetRawText();
            }
            throw new ManagerError(errString);
        }
        throw new ArgumentException($"Unexpected IPC response format: {response}");
    }
}

public class IPCResponse
{
    [JsonPropertyName("Ok")]
    public JsonElement Ok { get; set; }

    [JsonPropertyName("Err")]
    public JsonElement Err { get; set; }
}

class TunnelArgs
{
    public required ExitSelector Exit { get; set; }
}

class SetTunnelArgs : IIPCCommandArg
{
    public string CommandName() => "setTunnelArgs";
    public TunnelArgs? Args { get; set; }
    public required bool Active { get; set; }
}

class GetStatusArgs : IIPCCommandArg
{
    public string CommandName() => "getStatus";
    public string? KnownVersion { get; set; }
}

class GetExitListArgs : IIPCCommandArg
{
    public string CommandName() => "getExitList";
    public string? KnownVersion { get; set; }
}

public class StartTunnelCommand : IObscuraCommand
{
    public required string TunnelArgs { get; set; }
    public async Task<string> RunAsync()
    {
        var args = JsonSerializer.Deserialize<TunnelArgs>(TunnelArgs, JsonConfig.Options)
            ?? throw new ArgumentException($"Failed to parse tunnelArgs: {TunnelArgs}");
        return await IPCCommand.RunWithArgAsync(new SetTunnelArgs { Args = args, Active = true });
    }
}

public class StopTunnelCommand : IObscuraCommand
{
    public double? TimeoutMs { get; set; }

    public async Task<string> RunAsync()
    {
        using var cts = TimeoutMs is > 0 ? new CancellationTokenSource(TimeSpan.FromMilliseconds(TimeoutMs.Value)) : null;
        return await IPCCommand.RunWithArgAsync(new SetTunnelArgs { Active = false }, cts?.Token ?? default);
    }
}

public class RevealItemInDirCommand : IObscuraCommand
{
    private static readonly ILog Log = LogManager.GetLogger(typeof(RevealItemInDirCommand));
    public required string Path { get; set; }
    public Task<string> RunAsync()
    {
        try
        {
            Process.Start(new ProcessStartInfo
            {
                FileName = "explorer.exe",
                Arguments = $"/select,\"{Path}\"",
                UseShellExecute = true,
            });
        }
        catch (Exception ex)
        {
            Log.Error($"Reveal failed for {Path}: {ex}");
        }
        return IObscuraCommand.UnitResponse;
    }
}

public class DebugBundleCommand : IObscuraCommand
{
    private static readonly ILog Log = LogManager.GetLogger(typeof(DebugBundleCommand));
    public string? UserFeedback { get; set; }
    public async Task<string> RunAsync()
    {
        OsStatus.Instance.Update(s => s.DebugBundleStatus.Start());
        string? path = null;
        try
        {
            var bundleInfo = new BundleInfo();
            var resultJson = await IPCCommand.RunWithArgAsync(new CreateDebugBundleArgs
            {
                UserFeedback = UserFeedback,
                BundleInfo = bundleInfo,
            });
            path = JsonSerializer.Deserialize<string>(resultJson, JsonConfig.Options);
            if (path is not null)
            {
                await new RevealItemInDirCommand { Path = path }.RunAsync();
            }
        }
        catch (Exception ex)
        {
            Log.Error($"Failed to create debug bundle: {ex}");
            throw;
        }
        finally
        {
            OsStatus.Instance.Update(s =>
            {
                if (path is not null)
                {
                    s.DebugBundleStatus.SetPath(path);
                }
                else
                {
                    s.DebugBundleStatus.MarkError();
                }
                s.DebugBundleStatus.Finish();
            });
        }
        return JsonSerializer.Serialize(path);
    }
}

public class SendNotificationCommand : IObscuraCommand
{
    public required List<string> Texts { get; set; }

    public Task<string> RunAsync()
    {
        if (AppNotificationManager.IsSupported())
        {
            var manager = AppNotificationManager.Default;
            var builder = new AppNotificationBuilder();
            foreach (var text in Texts)
            {
                builder.AddText(text);
            }
            manager.Show(builder.BuildNotification());
        }
        return IObscuraCommand.UnitResponse;
    }
}
