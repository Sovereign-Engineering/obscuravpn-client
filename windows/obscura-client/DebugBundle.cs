using log4net;
using System;
using System.Collections.Generic;
using System.Diagnostics.CodeAnalysis;
using System.Globalization;
using System.Linq;
using System.Management;
using System.Runtime.InteropServices;
using System.Text.Json.Serialization;

namespace Obscura_Client;

class ComputerSystem
{
    readonly ushort ThermalState;
    readonly ulong TotalPhysicalMemory;

    public ComputerSystem(ManagementBaseObject item)
    {
        ThermalState = Convert.ToUInt16(item[nameof(ThermalState)]);
        TotalPhysicalMemory = Convert.ToUInt64(item[nameof(TotalPhysicalMemory)]);
    }

    public string GetThermalState()
    {
        return ThermalState switch
        {
            1 => "other",
            2 => "unknown",
            3 => "safe",
            4 => "warning",
            5 => "critical",
            _ => ThermalState.ToString()
        };
    }

    public double GetRAMPhysicalGiB()
    {
        return TotalPhysicalMemory / 1024.0 / 1024.0 / 1024.0;
    }
}

class OperatingSystem
{
    readonly string? Caption;
    readonly ulong FreePhysicalMemory;
    readonly DateTime LastBootUpTime;
    readonly string? Version;

    public OperatingSystem(ManagementBaseObject item)
    {
        Caption = item[nameof(Caption)] as string;
        FreePhysicalMemory = Convert.ToUInt64(item[nameof(FreePhysicalMemory)]);
        LastBootUpTime = ManagementDateTimeConverter.ToDateTime(item[nameof(LastBootUpTime)].ToString()).ToUniversalTime();
        Version = item[nameof(Version)] as string;
    }

    public string GetBootTimestamp()
    {
        return LastBootUpTime.ToString("yyyy-MM-dd'T'HH:mm:ssZ", CultureInfo.InvariantCulture);
    }

    public double GetRAMAvailableGiB()
    {
        return FreePhysicalMemory / 1024.0 / 1024.0;
    }

    // `Caption` = marketing name (i.e. Microsoft Windows 11 Pro)
    // `Version` = kernel version (this is still 10.x.y even on Windows 11)
    // `RuntimeInformation.OSDescription` = Microsoft Windows + kernel version
    public string GetOSVersionString()
    {
        return $"{Caption} ({Version})";
    }
}

class Processor
{
    public readonly string? Name;
    public readonly uint NumberOfCores;

    public Processor(ManagementBaseObject item)
    {
        Name = item[nameof(Name)] as string;
        NumberOfCores = Convert.ToUInt32(item[nameof(NumberOfCores)]);
    }
}

// TODO: Switch to `JsonNamingPolicyAttribute` when we update to .NET 11
// https://linear.app/soveng/issue/OBS-3949/use-jsonnamingpolicyattribute-when-were-on-net-11
public class BundleInfo
{
    private static readonly ILog Log = LogManager.GetLogger(typeof(BundleInfo));
    [JsonPropertyName("AppVersion")]
    public required string AppVersion { get; set; }
    [JsonPropertyName("BootTimestamp")]
    public string? BootTimestamp { get; set; }
    [JsonPropertyName("DotNETFramework")]
    public string? DotNETFramework { get; set; }
    [JsonPropertyName("OSArchitecture")]
    public string? OSArchitecture { get; set; }
    [JsonPropertyName("OSVersionString")]
    public string? OSVersionString { get; set; }
    [JsonPropertyName("PID")]
    public int? PID { get; set; }
    [JsonPropertyName("ProcessArchitecture")]
    public string? ProcessArchitecture { get; set; }
    [JsonPropertyName("ProcessPath")]
    public string? ProcessPath { get; set; }
    [JsonPropertyName("ProcessorCountActive")]
    public int? ProcessorCountActive { get; set; }
    [JsonPropertyName("ProcessorCountPhysical")]
    public int? ProcessorCountPhysical { get; set; }
    [JsonPropertyName("ProcessorName")]
    public string? ProcessorName { get; set; }
    [JsonPropertyName("RAMAvailableGiB")]
    public double? RAMAvailableGiB { get; set; }
    [JsonPropertyName("RAMPhysicalGiB")]
    public double? RAMPhysicalGiB { get; set; }
    [JsonPropertyName("ThermalState")]
    public string? ThermalState { get; set; }
    [JsonPropertyName("UptimeHours")]
    public double? UptimeHours { get; set; }

    static T? Query<T>(string query, Func<IEnumerable<ManagementBaseObject>, T?> f)
    {
        var output = default(T);
        try
        {
            using var searcher = new ManagementObjectSearcher(query);
            using var results = searcher.Get();
            output = f(results.Cast<ManagementBaseObject>());
        }
        catch (Exception ex)
        {
            Log.Error($"Query `{query}` failed: {ex.Message}");
        }
        return output;
    }

    [SetsRequiredMembers]
    public BundleInfo()
    {
        var computerSystem = Query(
            "SELECT ThermalState, TotalPhysicalMemory FROM Win32_ComputerSystem",
            items => items.Select(item => new ComputerSystem(item)).First()
        );
        var operatingSystem = Query(
            "SELECT Caption, FreePhysicalMemory, LastBootUpTime, Version FROM Win32_OperatingSystem",
            items => items.Select(item => new OperatingSystem(item)).First()
        );
        var processors = Query(
            "SELECT Name, NumberOfCores FROM Win32_Processor",
            items => items.Select(item => new Processor(item)).ToList()
        );
        AppVersion = OsStatus.GetSrcVersion();
        BootTimestamp = operatingSystem?.GetBootTimestamp();
        DotNETFramework = RuntimeInformation.FrameworkDescription;
        // OS architecture may differ from process architecture:
        // https://learn.microsoft.com/en-us/dotnet/api/system.runtime.interopservices.runtimeinformation.osarchitecture?view=net-10.0#remarks
        OSArchitecture = RuntimeInformation.OSArchitecture.ToString();
        OSVersionString = operatingSystem?.GetOSVersionString() ?? RuntimeInformation.OSDescription;
        PID = Environment.ProcessId;
        ProcessArchitecture = RuntimeInformation.ProcessArchitecture.ToString();
        ProcessPath = Environment.ProcessPath;
        ProcessorCountActive = Environment.ProcessorCount;
        ProcessorCountPhysical = processors?.Sum(processor => Convert.ToInt32(processor.NumberOfCores));
        ProcessorName = processors?.FirstOrDefault()?.Name;
        RAMAvailableGiB = operatingSystem?.GetRAMAvailableGiB();
        RAMPhysicalGiB = computerSystem?.GetRAMPhysicalGiB();
        ThermalState = computerSystem?.GetThermalState();
        UptimeHours = TimeSpan.FromMilliseconds(Environment.TickCount64).TotalHours;
    }
}

public class CreateDebugBundleArgs : IIPCCommandArg
{
    public string CommandName() => "createDebugBundle";
    public string? UserFeedback { get; set; }
    public required BundleInfo BundleInfo { get; set; }
}
