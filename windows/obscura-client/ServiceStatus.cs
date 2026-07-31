using System;
using System.ComponentModel;
using System.Diagnostics;
using System.Management;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Threading.Tasks;
using log4net;

namespace Obscura_Client;

public enum WindowsServiceDegradation
{
    Stopped,
    Failed,
    Disabled,
    NotInstalled,
    Other,
}

// Union: exactly one variant is set; Initializing when both are null.
[JsonConverter(typeof(ServiceStatusJsonConverter))]
public sealed class ServiceStatus
{
    public NeStatus? Healthy { get; init; }
    public DegradedServiceInfo? Degraded { get; init; }

    public static readonly ServiceStatus Initializing = new();
}

public sealed class ServiceStatusJsonConverter : JsonConverter<ServiceStatus>
{
    public override ServiceStatus Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options) =>
        throw new NotSupportedException();

    public override void Write(Utf8JsonWriter writer, ServiceStatus value, JsonSerializerOptions options)
    {
        if (value.Healthy is { } healthy)
        {
            writer.WriteStartObject();
            writer.WritePropertyName("healthy");
            JsonSerializer.Serialize(writer, healthy, options);
            writer.WriteEndObject();
        }
        else if (value.Degraded is { } degraded)
        {
            writer.WriteStartObject();
            writer.WritePropertyName("degraded");
            JsonSerializer.Serialize(writer, degraded, options);
            writer.WriteEndObject();
        }
        else
        {
            writer.WriteStringValue("initializing");
        }
    }
}

public sealed class DegradedServiceInfo
{
    public NeStatus? LastStatus { get; set; }
    public WindowsServiceDegradation WindowsDegradation { get; set; }
}

public enum WindowsFixAction
{
    Start,
    EnableAndStart,
}

public static class ObscuraService
{
    private static readonly ILog Log = LogManager.GetLogger(typeof(ObscuraService));
    public const string Name = "Obscura VPN Service";

    const int ERROR_CANCELLED = 1223;
    const int ERROR_SERVICE_ALREADY_RUNNING = 1056;

    public static WindowsServiceDegradation Diagnose()
    {
        try
        {
            using var searcher = new ManagementObjectSearcher(
                $"SELECT State, StartMode FROM Win32_Service WHERE Name = '{Name}'");
            using var results = searcher.Get();
            foreach (var obj in results)
            {
                using (obj)
                {
                    var state = obj["State"] as string;
                    var startMode = obj["StartMode"] as string;
                    if (string.Equals(state, "Running", StringComparison.OrdinalIgnoreCase))
                    {
                        // Installed and running, yet the pipe is unresponsive.
                        return WindowsServiceDegradation.Other;
                    }
                    if (string.Equals(startMode, "Disabled", StringComparison.OrdinalIgnoreCase))
                    {
                        return WindowsServiceDegradation.Disabled;
                    }
                    return WindowsServiceDegradation.Stopped;
                }
            }
            return WindowsServiceDegradation.NotInstalled;
        }
        catch (Exception ex)
        {
            Log.Warn($"could not classify service degradation: {ex.Message}");
            return WindowsServiceDegradation.Other;
        }
    }

    public static async Task ApplyFixAsync(WindowsFixAction action)
    {
        try
        {
            await (action switch
            {
                WindowsFixAction.Start =>
                    RunElevatedAsync("sc.exe", $"start \"{Name}\""),
                WindowsFixAction.EnableAndStart =>
                    RunElevatedAsync("cmd.exe", $"/d /c sc config \"{Name}\" start= auto && sc start \"{Name}\""),
                _ => throw new NotSupportedException($"unsupported service fix on Windows: {action}"),
            });
            Log.Info($"service fix succeeded: {action}");
        }
        catch (Exception ex)
        {
            Log.Error($"service fix failed: {action}: {ex.Message}");
            throw;
        }
    }

    static async Task RunElevatedAsync(string fileName, string arguments)
    {
        Log.Info($"running elevated service fix: {fileName} {arguments}");
        Process process;
        try
        {
            process = Process.Start(new ProcessStartInfo
            {
                FileName = fileName,
                Arguments = arguments,
                UseShellExecute = true,
                Verb = "runas",
                WindowStyle = ProcessWindowStyle.Hidden,
            }) ?? throw new InvalidOperationException("failed to start elevated process");
        }
        catch (Win32Exception ex) when (ex.NativeErrorCode == ERROR_CANCELLED)
        {
            throw new InvalidOperationException("Administrator approval was declined.", ex);
        }
        using (process)
        {
            await process.WaitForExitAsync();
            if (process.ExitCode != 0 && process.ExitCode != ERROR_SERVICE_ALREADY_RUNNING)
            {
                Log.Error($"service fix '{fileName} {arguments}' exited with {process.ExitCode}");
                throw new InvalidOperationException($"Could not start the service (exit code {process.ExitCode}).");
            }
        }
    }
}
