param(
    [string]$Version,
    [string]$OutDir = (Join-Path $PSScriptRoot 'build'),
    # KeyLocker keypair alias; when set, signs via smctl (MSBuild callers pass /p:SignKeypairAlias).
    [string]$KeypairAlias,
    [string]$SelfSignedPfx = (Join-Path $PSScriptRoot '..\obscura-client\obscura-client_TemporaryKey.pfx'),
    [SecureString]$SelfSignedPfxPassword = $env:OBSCURA_PFX_PASSWORD,
    # Skip signing entirely (produce an unsigned .msix). CI passes this on non-tag builds, where no
    # signing cert is available -- signing is gated to tag builds.
    [switch]$SkipSign
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
Set-PSDebug -Trace 1

# EV / KeyLocker when a keypair alias is provided (CI / production), otherwise local self-signed.
# Mirrors sign.ps1, which does the actual signing.
$useKeyLocker = -not [string]::IsNullOrWhiteSpace($KeypairAlias)

$clientDir = Join-Path $PSScriptRoot '..\obscura-client'
$manifestSrc = Join-Path $clientDir 'SparsePackage.appxmanifest.xml'
$assetsSrc = Join-Path $clientDir 'Assets'
$tagJson = Join-Path $PSScriptRoot '..\..\tag.json'
$staging = Join-Path $OutDir 'sparse-staging'

# Resolve a Windows SDK tool (MakeAppx.exe) from the newest installed SDK bin dir.
function Resolve-SdkTool([string]$name, [string]$override) {
    if ($override) {
        if (-not (Test-Path -LiteralPath $override)) { throw "$name not found at: $override" }
        return $override
    }
    $found = Get-Command $name -ErrorAction SilentlyContinue
    if ($found) { return $found.Source }
    $roots = @("${env:ProgramFiles(x86)}\Windows Kits\10\bin", "$env:ProgramFiles\Windows Kits\10\bin")
    $tool = Get-ChildItem -Path $roots -Recurse -Filter $name -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match '\\x64\\' } |
        Sort-Object FullName -Descending | Select-Object -First 1
    if (-not $tool) { throw "$name not found. Install the Windows SDK or pass an explicit path." }
    return $tool.FullName
}

# Normalize an arbitrary version string to the 4-part Major.Minor.Build.Revision MSIX form.
function Get-FourPartVersion([string]$v) {
    $parts = @($v -split '\.' | Where-Object { $_ -ne '' })
    while ($parts.Count -lt 4) { $parts += '0' }
    return ($parts[0..3] -join '.')
}

if (-not $Version) { $Version = $env:OBSCURA_VERSION }
if (-not $Version) {
    $Version = ([regex]'"version":\s*"([^"]+)"').Match((Get-Content -LiteralPath $tagJson -Raw)).Groups[1].Value
}
$version4 = Get-FourPartVersion $Version
Write-Host "Sparse package version: $version4"

# Stage a copy of the manifest with the Version attribute stamped.
New-Item -ItemType Directory -Force -Path $staging | Out-Null
[xml]$manifest = Get-Content -LiteralPath $manifestSrc
$manifest.Package.Identity.Version = $version4

# A package's signature publisher must equal Identity/Publisher. In self-signed mode the .msix is
# signed with the local .pfx, whose subject differs from the production EV subject, so stamp the
# manifest with the .pfx's actual subject. (KeyLocker mode and unsigned builds keep the committed EV publisher.)
if (-not $SkipSign -and -not $useKeyLocker) {
    if (-not (Test-Path -LiteralPath $SelfSignedPfx)) { throw "Self-signed .pfx not found: $SelfSignedPfx" }
    $cert = [System.Security.Cryptography.X509Certificates.X509Certificate2]::new($SelfSignedPfx, [string]$SelfSignedPfxPassword)
    $manifest.Package.Identity.Publisher = $cert.Subject
    Write-Host "Self-signed publisher: $($cert.Subject)"
}

$stagedManifest = Join-Path $staging 'AppxManifest.xml'
$manifest.Save($stagedManifest)

# Copy Assets dir
if (-not (Test-Path -LiteralPath $assetsSrc)) { throw "Assets not found: $assetsSrc" }
Copy-Item -LiteralPath $assetsSrc -Destination $staging -Recurse -Force

$makeAppxExe = Resolve-SdkTool 'MakeAppx.exe'
$msixOut = Join-Path $OutDir 'Obscura.msix'
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

Write-Host "Packing $msixOut"
& $makeAppxExe pack /o /d $staging /nv /p $msixOut

# Sign the packed .msix. sign.ps1 picks EV/KeyLocker vs self-signed the same way we did above.
if ($SkipSign) {
    Write-Host "Skipping signing (-SkipSign): $msixOut is unsigned"
} else {
    & (Join-Path $PSScriptRoot 'sign.ps1') -Files $msixOut -KeypairAlias $KeypairAlias -SelfSignedPfx $SelfSignedPfx -SelfSignedPfxPassword $SelfSignedPfxPassword
}

Write-Host "Done: $msixOut"
