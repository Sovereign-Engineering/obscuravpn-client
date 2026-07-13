using log4net;
using System;
using System.Collections.Generic;
using System.Diagnostics.CodeAnalysis;
using System.Globalization;
using System.Linq;
using System.Management;
using System.Runtime.InteropServices;

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

    public double GetMemoryTotalGib()
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

    public double GetMemoryAvailGib()
    {
        return FreePhysicalMemory / 1024.0 / 1024.0;
    }

    // `Caption` = marketing name (i.e. Microsoft Windows 11 Pro)
    // `Version` = kernel version (this is still 10.x.y even on Windows 11)
    // `RuntimeInformation.OSDescription` = Microsoft Windows + kernel version
    public string GetOsVersion()
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

public class BundleInfo
{
    private static readonly ILog Log = LogManager.GetLogger(typeof(BundleInfo));
    public required string AppVersion { get; set; }
    public string? BootTimestamp { get; set; }
    public string? DotnetFramework { get; set; }
    public double? MemoryAvailGib { get; set; }
    public double? MemoryTotalGib { get; set; }
    public string? OsArchitecture { get; set; }
    public string? OsVersion { get; set; }
    public string? ProcessArchitecture { get; set; }
    public int? ProcessId { get; set; }
    public string? ProcessPath { get; set; }
    public int? ProcessorCountActive { get; set; }
    public int? ProcessorCountPhysical { get; set; }
    public string? ProcessorName { get; set; }
    public string? ThermalState { get; set; }
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
        DotnetFramework = RuntimeInformation.FrameworkDescription;
        MemoryAvailGib = operatingSystem?.GetMemoryAvailGib();
        MemoryTotalGib = computerSystem?.GetMemoryTotalGib();
        // OS architecture may differ from process architecture:
        // https://learn.microsoft.com/en-us/dotnet/api/system.runtime.interopservices.runtimeinformation.osarchitecture?view=net-10.0#remarks
        OsArchitecture = RuntimeInformation.OSArchitecture.ToString();
        OsVersion = operatingSystem?.GetOsVersion() ?? RuntimeInformation.OSDescription;
        ProcessArchitecture = RuntimeInformation.ProcessArchitecture.ToString();
        ProcessId = Environment.ProcessId;
        ProcessPath = Environment.ProcessPath;
        ProcessorCountActive = Environment.ProcessorCount;
        ProcessorCountPhysical = processors?.Sum(processor => Convert.ToInt32(processor.NumberOfCores));
        ProcessorName = processors?.FirstOrDefault()?.Name;
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
