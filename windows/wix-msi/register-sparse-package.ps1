# Registers the sparse identity package. Run by the MSI's RegisterSparsePackage* custom actions in
# two modes:
#   (default)     LocalSystem: stage (must succeed), then best-effort provision for all users.
#   -CurrentUser  impersonated as the installing user: register for that user only (no elevation).
#
# Machine-wide provisioning uses the PackageManager WinRT API rather than DISM's
# Add-AppxProvisionedPackage: DISM re-validates the .msix and rejects external-location packages on
# builds predating its -ExternalLocationPath support. Provisioning is best-effort regardless - an
# application-control policy (Smart App Control, WDAC) can reject all-user provisioning with
# 0xC1570108 - so the -CurrentUser registration below (which always runs) is what guarantees the
# installing user gets identity, and the app re-registers per-user on launch as a last resort.
param([switch]$CurrentUser)

$ErrorActionPreference = 'Stop'
$installDir = $PSScriptRoot
$packageName = 'SovereignEngineering.ObscuraVPN'
$msix = Join-Path $installDir 'Obscura.msix'

if ($CurrentUser) {
    # Per-user registration for the installing user; no elevation, different operation from all-user
    # provisioning, so it can succeed where that was blocked by policy.
    Add-AppxPackage -Path $msix -ExternalLocation $installDir
    return
}

# Required: without a staged package the app has no identity to register against.
Add-AppxPackage -Stage $msix -ExternalLocation $installDir

$staged = Get-AppxPackage -AllUsers -Name $packageName | Select-Object -First 1
if (-not $staged) { throw "$packageName is not staged after Add-AppxPackage -Stage" }

# PowerShell 5.1 surfaces WinRT async operations as bare COM objects; await them through AsTask.
[Windows.Management.Deployment.PackageManager, Windows.Management.Deployment, ContentType=WindowsRuntime] | Out-Null
Add-Type -AssemblyName System.Runtime.WindowsRuntime
$asTask = [System.WindowsRuntimeSystemExtensions].GetMethods() |
    Where-Object { $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperationWithProgress`2' } |
    Select-Object -First 1
$asTask = $asTask.MakeGenericMethod([Windows.Management.Deployment.DeploymentResult], [Windows.Management.Deployment.DeploymentProgress])

# Best-effort: a policy can reject all-user provisioning even when staging succeeded. Don't fail the
# install - the -CurrentUser CA and the app's self-repair cover per-user identity.
try {
    $op = (New-Object Windows.Management.Deployment.PackageManager).ProvisionPackageForAllUsersAsync($staged.PackageFamilyName)
    $result = $asTask.Invoke($null, @($op)).GetAwaiter().GetResult()
    if ($result.ExtendedErrorCode) {
        Write-Host ('Provisioning {0} failed (non-fatal): 0x{1:X8} {2}' -f $staged.PackageFamilyName, $result.ExtendedErrorCode.HResult, $result.ErrorText)
    } else {
        Write-Host "Provisioned $($staged.PackageFamilyName) for all users"
    }
} catch {
    Write-Host "Provisioning $($staged.PackageFamilyName) failed (non-fatal): $($_.Exception.Message)"
}
