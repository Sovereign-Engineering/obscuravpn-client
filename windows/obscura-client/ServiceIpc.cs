using System;
using System.IO;
using System.IO.Pipes;
using System.Threading;
using System.Threading.Tasks;
using log4net;

namespace Obscura_Client;

/// <summary>
/// Named pipe client for communicating with the Obscura Rust service.
/// Protocol: length-prefixed JSON (4-byte big-endian u32 length + UTF-8 JSON body) in both directions.
/// </summary>
public static class ServiceIpc
{
    private static readonly ILog Log = LogManager.GetLogger(typeof(ServiceIpc));
    private const string PIPE_NAME = "obscuravpn";
    private const uint MAX_MESSAGE_LEN = 1_000_000;

    /// <summary>
    /// Sends a JSON command to the Rust service over a named pipe and returns the JSON response.
    /// Each call creates a new pipe connection (matching the one-connection-per-command model).
    /// <paramref name="commandJson"/> must be a valid ManagerCmd JSON string (e.g. {"getStatus":{"knownVersion":null}}).
    /// </summary>
    public static async Task<string> SendCommand(string commandJson, CancellationToken cancellationToken = default)
    {
        await using var pipe = new NamedPipeClientStream(".", PIPE_NAME, PipeDirection.InOut, PipeOptions.Asynchronous);

        try
        {
            await pipe.ConnectAsync(5000, cancellationToken);
        }
        catch (Exception ex)
        {
            Log.Error($"[ServiceIpc] Failed to connect to pipe: {ex}");
            throw;
        }

        // Send length-prefixed command
        byte[] commandBytes;
        try
        {
            commandBytes = System.Text.Encoding.UTF8.GetBytes(commandJson);
        }
        catch (Exception ex)
        {
            Log.Error($"[ServiceIpc] Failed to serialize command: {ex}");
            throw;
        }

        var len = (uint)commandBytes.Length;
        if (len > MAX_MESSAGE_LEN)
        {
            throw new ArgumentException($"Command length {len} exceeds maximum allowed size {MAX_MESSAGE_LEN}", nameof(commandJson));
        }

        var lengthBytes = new byte[4];
        lengthBytes[0] = (byte)(len >> 24);
        lengthBytes[1] = (byte)(len >> 16);
        lengthBytes[2] = (byte)(len >> 8);
        lengthBytes[3] = (byte)len;

        try
        {
            await pipe.WriteAsync(lengthBytes, cancellationToken);
        }
        catch (Exception ex)
        {
            Log.Error($"[ServiceIpc] Failed to write command length: {ex}");
            throw;
        }

        try
        {
            await pipe.WriteAsync(commandBytes, cancellationToken);
        }
        catch (Exception ex)
        {
            Log.Error($"[ServiceIpc] Failed to write command body: {ex}");
            throw;
        }

        try
        {
            await pipe.FlushAsync(cancellationToken);
        }
        catch (Exception ex)
        {
            Log.Error($"[ServiceIpc] Failed to flush pipe: {ex}");
            throw;
        }

        // Read length-prefixed response
        var responseLengthBytes = new byte[4];
        try
        {
            await pipe.ReadExactlyAsync(responseLengthBytes, cancellationToken);
        }
        catch (Exception ex)
        {
            Log.Error($"[ServiceIpc] Failed to read response length: {ex}");
            throw;
        }

        var responseLength =
            (uint)responseLengthBytes[0] << 24 |
            (uint)responseLengthBytes[1] << 16 |
            (uint)responseLengthBytes[2] << 8 |
            responseLengthBytes[3];

        if (responseLength > 10_000_000)
        {
            throw new InvalidDataException($"Response length {responseLength} exceeds maximum allowed size");
        }

        var responseBytes = new byte[responseLength];
        try
        {
            await pipe.ReadExactlyAsync(responseBytes, cancellationToken);
        }
        catch (Exception ex)
        {
            Log.Error($"[ServiceIpc] Failed to read response body: {ex}");
            throw;
        }

        try
        {
            return System.Text.Encoding.UTF8.GetString(responseBytes);
        }
        catch (Exception ex)
        {
            Log.Error($"[ServiceIpc] Failed to decode response: {ex}");
            throw;
        }
    }
}
