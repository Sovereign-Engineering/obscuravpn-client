using System;
using System.Collections.Generic;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization;
using System.Threading;
using System.Threading.Tasks;
using log4net;

namespace Obscura_Client;

public class InvokeCommand
{
    private static readonly ILog Log = LogManager.GetLogger(typeof(InvokeCommand));

    public GetOsStatusCommand? GetOsStatus { get; set; }
    public SetNavigationViewCommand? SetNavigationView { get; set; }
    public IPCCommand? JsonFfiCmd { get; set; }
    public StartTunnelCommand? StartTunnel { get; set; }
    public StopTunnelCommand? StopTunnel { get; set; }
    public RevealItemInDirCommand? RevealItemInDir { get; set; }
    public DebuggingArchiveCommand? DebuggingArchive { get; set; }

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
        if (invoke.JsonFfiCmd != null) return invoke.JsonFfiCmd;
        if (invoke.StartTunnel != null) return invoke.StartTunnel;
        if (invoke.StopTunnel != null) return invoke.StopTunnel;
        if (invoke.RevealItemInDir != null) return invoke.RevealItemInDir;
        if (invoke.DebuggingArchive != null) return invoke.DebuggingArchive;
        Log.Warn($"Unknown command: {commandJson}");
        throw new NotSupportedException($"Unknown command: {commandJson}");
    }
}

public interface IObscuraCommand
{
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
        OsStatus.Instance.SetNavigationView(View);
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
    public string Cmd { get; set; } = "";
    public double? TimeoutMs { get; set; }

    public static async Task<string> RunWithArgAsync(IIPCCommandArg command)
    {
        var wrapped = new Dictionary<string, object?> { [command.CommandName()] = command };
        return await new IPCCommand { Cmd = JsonSerializer.Serialize(wrapped, JsonConfig.Options) }.RunAsync();
    }

    public async Task<string> RunAsync()
    {
        if (string.IsNullOrEmpty(Cmd))
            throw new ArgumentException("jsonFfiCmd missing required `cmd` string property");

        using var cts = TimeoutMs is > 0 ? new CancellationTokenSource(TimeSpan.FromMilliseconds(TimeoutMs.Value)) : null;
        var cancellationToken = cts?.Token ?? default;

        var response = await ServiceIpc.SendCommand(Cmd, cancellationToken);

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

class SetTunnelArgs : IIPCCommandArg
{
    public string CommandName() => "setTunnelArgs";
    public JsonNode? Args { get; set; }
    public required bool Active { get; set; }
}

public class StartTunnelCommand : IObscuraCommand
{
    public required string TunnelArgs { get; set; }
    public async Task<string> RunAsync()
    {
        return await IPCCommand.RunWithArgAsync(new SetTunnelArgs { Args = JsonNode.Parse(TunnelArgs), Active = true });
    }
}

public class StopTunnelCommand : IObscuraCommand
{
    public async Task<string> RunAsync()
    {
        return await IPCCommand.RunWithArgAsync(new SetTunnelArgs { Active = false });
    }
}

public class RevealItemInDirCommand : IObscuraCommand
{
    public required string Path { get; set; }
    public Task<string> RunAsync()
    {
        throw new NotImplementedException();
    }
}

public class DebuggingArchiveCommand : IObscuraCommand, IIPCCommandArg
{
    public string CommandName() => "createDebugArchive";
    public string? UserFeedback { get; set; }
    async public Task<string> RunAsync()
    {
        // TODO: this updates OsStatus
        var archive = await IPCCommand.RunWithArgAsync(this);
        throw new NotImplementedException();
    }
}
