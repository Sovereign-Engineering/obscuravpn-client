using log4net;
using System;
using System.Collections.Generic;
using System.Diagnostics.CodeAnalysis;
using System.Globalization;
using System.IO;
using System.IO.Compression;
using System.Linq;
using System.Management;
using System.Runtime.InteropServices;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;

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

public class BundleInfo
{
    private static readonly ILog Log = LogManager.GetLogger(typeof(BundleInfo));
    public required string AppVersion { get; set; }
    public string? BootTimestamp { get; set; }
    public string? BundleTimestamp { get; set; }
    public string? DotNETFramework { get; set; }
    public bool HasIdentity { get; set; }
    public string? OSArchitecture { get; set; }
    public string? OSVersionString { get; set; }
    public int? PID { get; set; }
    public string? ProcessArchitecture { get; set; }
    public string? ProcessPath { get; set; }
    public int? ProcessorCountActive { get; set; }
    public int? ProcessorCountPhysical { get; set; }
    public string? ProcessorName { get; set; }
    public double? RAMAvailableGiB { get; set; }
    public double? RAMPhysicalGiB { get; set; }
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
        HasIdentity = PackageIdentity.IsPackagedProcess();
    }
}

public class CreateServiceDebugBundleArgs : IIPCCommandArg
{
    public string CommandName() => "createServiceDebugBundle";
}

public class DeleteServiceDebugBundleArgs : IIPCCommandArg
{
    public string CommandName() => "deleteServiceDebugBundle";
    public required string Token { get; set; }
}

public class ServiceDebugBundleHandle
{
    public required string Path { get; set; }
    public required string Token { get; set; }
}

public class DebugBundleInProgressException() : Exception("debugBundleInProgress");

public static partial class DebugBundle
{
    private static readonly ILog Log = LogManager.GetLogger(typeof(DebugBundle));
    const string DirPrefix = "obscura-debug-bundle-";
    static readonly SemaphoreSlim Gate = new(1, 1);

    public static IDisposable Reserve()
    {
        if (!Gate.Wait(0))
        {
            Log.Warn("Debug bundle already in progress, rejecting request");
            throw new DebugBundleInProgressException();
        }
        return new Reservation();
    }

    sealed partial class Reservation : IDisposable
    {
        public void Dispose() => Gate.Release();
    }

    public static async Task<string> CreateAsync(string? userFeedback)
    {
        var timestampUtc = DateTime.UtcNow;
        var bundleName = DirPrefix + timestampUtc.ToString("yyyy-MM-dd'T'HH-mm-ss'Z'", CultureInfo.InvariantCulture);
        var debugBundlesDir = Path.Combine(Path.GetTempPath(), "debug-bundles");
        Directory.CreateDirectory(debugBundlesDir);

        var stagingDir = Directory.CreateTempSubdirectory(DirPrefix).FullName;
        var zipPath = Path.Combine(debugBundlesDir, bundleName + ".zip");
        try
        {
            await CollectServiceBundleAsync(stagingDir);
            await Task.Run(() =>
            {
                PopulateClientBundle(stagingDir, userFeedback, timestampUtc);
                ZipFile.CreateFromDirectory(stagingDir, zipPath);
            });
        }
        finally
        {
            DeleteBundleDir(stagingDir);
        }
        return zipPath;
    }

    // The service hands out a dir that is read-only to us, so copy it into our own staging dir
    // and hand the original back for deletion.
    static async Task CollectServiceBundleAsync(string stagingDir)
    {
        ServiceDebugBundleHandle handle;
        try
        {
            var resultJson = await IPCCommand.RunWithArgAsync(new CreateServiceDebugBundleArgs());
            handle = JsonSerializer.Deserialize<ServiceDebugBundleHandle>(resultJson, JsonConfig.Options)
                ?? throw new InvalidOperationException($"createServiceDebugBundle returned null body: {resultJson}");
        }
        catch (Exception ex)
        {
            Log.Error($"Service failed to create its debug bundle: {ex}");
            return;
        }
        try
        {
            await Task.Run(() => CopyDirContents(handle.Path, stagingDir));
        }
        catch (Exception ex)
        {
            Log.Error($"Failed to copy service debug bundle {handle.Path} into {stagingDir}: {ex}");
        }
        try
        {
            await IPCCommand.RunWithArgAsync(new DeleteServiceDebugBundleArgs { Token = handle.Token });
        }
        catch (Exception ex)
        {
            Log.Error($"Service failed to delete its debug bundle: {ex.Message}");
        }
    }

    static void PopulateClientBundle(string dir, string? userFeedback, DateTime timestampUtc)
    {
        try
        {
            var info = new BundleInfo
            {
                BundleTimestamp = timestampUtc.ToString("yyyy-MM-dd'T'HH:mm:ss'Z'", CultureInfo.InvariantCulture),
            };
            File.WriteAllText(Path.Combine(dir, "info.json"), JsonSerializer.Serialize(info, JsonConfig.BundleInfoOptions));
        }
        catch (Exception ex)
        {
            Log.Error($"Failed to write info.json into debug bundle: {ex}");
        }
        if (!string.IsNullOrEmpty(userFeedback))
        {
            try
            {
                File.WriteAllText(Path.Combine(dir, "user-feedback.txt"), userFeedback);
            }
            catch (Exception ex)
            {
                Log.Error($"Failed to write user feedback into debug bundle: {ex}");
            }
        }
        try
        {
            CopyDirContents(App.ClientLogDir, Path.Combine(dir, "logs-client"));
        }
        catch (Exception ex)
        {
            Log.Error($"Failed to copy client logs into debug bundle: {ex}");
        }
    }

    static void CopyDirContents(string src, string dst)
    {
        Directory.CreateDirectory(dst);
        foreach (var srcPath in Directory.EnumerateFileSystemEntries(src))
        {
            var dstPath = Path.Combine(dst, Path.GetFileName(srcPath));
            try
            {
                if (Directory.Exists(srcPath))
                {
                    CopyDirContents(srcPath, dstPath);
                }
                else
                {
                    // The active log file is held open by the appender, so share the read
                    using var srcStream = new FileStream(srcPath, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
                    using var dstStream = File.Create(dstPath);
                    srcStream.CopyTo(dstStream);
                }
            }
            catch (Exception ex)
            {
                Log.Error($"Failed to copy {srcPath} into debug bundle: {ex.Message}");
            }
        }
    }

    static void DeleteBundleDir(string bundleDir)
    {
        try
        {
            if (Directory.Exists(bundleDir))
            {
                Directory.Delete(bundleDir, true);
            }
        }
        catch (Exception ex)
        {
            Log.Error($"Failed to delete debug bundle dir {bundleDir}: {ex.Message}");
        }
    }
}
