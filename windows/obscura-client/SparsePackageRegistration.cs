using log4net;
using System;
using System.IO;
using System.Linq;
using System.Security.Principal;
using System.Threading.Tasks;
using Windows.Management.Deployment;

namespace Obscura_Client;

// Registers the sparse identity package for the current user when the installer's machine-wide
// provisioning was skipped or blocked. Per-user registration needs no elevation and is a different
// operation from all-user provisioning, so it can succeed where that was refused by policy.
internal static class SparsePackageRegistration
{
    private static readonly ILog Log = LogManager.GetLogger(typeof(SparsePackageRegistration));

    private const string PackageName = "SovereignEngineering.ObscuraVPN";

    // Obscura.msix and the app binaries are installed side by side, so the package's external
    // location is the app directory itself.
    private static string PackagePath => Path.Combine(AppContext.BaseDirectory, "Obscura.msix");

    public static bool CanRegister => File.Exists(PackagePath);

    // True once the sparse package is registered for the current user (identity attaches at the next
    // process start). Read straight from the package store, so it reflects a registration that
    // completed service-side even if the AddPackageByUriAsync call hasn't returned yet.
    public static bool IsRegisteredForCurrentUser()
    {
        try
        {
            var sid = WindowsIdentity.GetCurrent().User?.Value;
            if (sid is null) return false;
            return new PackageManager().FindPackagesForUser(sid).Any(p => p.Id.Name == PackageName);
        }
        catch (Exception ex)
        {
            Log.Warn($"Checking package registration failed: {ex.Message}");
            return false;
        }
    }

    public static async Task RegisterForCurrentUserAsync()
    {
        Log.Info($"Registering identity package {PackagePath} for current user");
        // Run on a background thread: awaiting the WinRT deployment operation directly on the UI
        // thread hangs (the operation completes but the continuation never resumes on that context).
        var result = await Task.Run(async () =>
        {
            // CA1416: ExternalLocationUri/AddPackageByUriAsync need Windows 10 19041+, which the
            // installer's build Launch condition and the sparse package both require, so this only ever
            // runs there. A runtime OperatingSystem.IsWindowsVersionAtLeast guard is avoided - it trips
            // an NRE in the WinAppSDK 2.3 XAML compiler when any project source uses it.
#pragma warning disable CA1416
            var options = new AddPackageOptions { ExternalLocationUri = new Uri(AppContext.BaseDirectory) };
            var op = new PackageManager().AddPackageByUriAsync(new Uri(PackagePath), options);
            op.Progress = (_, p) => Log.Info($"Registration progress: {p.state} {p.percentage}%");
            return await op;
#pragma warning restore CA1416
        });
        if (result.ExtendedErrorCode != null)
        {
            throw new InvalidOperationException(
                $"AddPackageByUriAsync failed: {result.ErrorText} (0x{result.ExtendedErrorCode.HResult:X8})");
        }
        Log.Info("Identity package registered for current user");
    }
}
