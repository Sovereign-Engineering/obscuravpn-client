param(
    [Parameter(Mandatory = $true)][string[]]$Files,
    # KeyLocker keypair alias; when set, signs via smctl (MSBuild callers pass /p:SignKeypairAlias).
    [string]$KeypairAlias,
    [string]$SelfSignedPfx,
    [SecureString]$SelfSignedPfxPassword = $env:OBSCURA_PFX_PASSWORD
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
Set-PSDebug -Trace 1

if (-not $SelfSignedPfx) {
    $scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
    $SelfSignedPfx = Join-Path $scriptDir '..\obscura-client\obscura-client_TemporaryKey.pfx'
}

# Resolve signtool.exe from PATH or the newest installed Windows SDK bin dir.
function Resolve-SignTool() {
    $found = Get-Command 'signtool.exe' -ErrorAction SilentlyContinue
    if ($found) { return $found.Source }
    $roots = @("${env:ProgramFiles(x86)}\Windows Kits\10\bin", "$env:ProgramFiles\Windows Kits\10\bin")
    $tool = Get-ChildItem -Path $roots -Recurse -Filter 'signtool.exe' -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match '\\x64\\' } |
        Sort-Object FullName -Descending | Select-Object -First 1
    if (-not $tool) { throw "signtool.exe not found. Install the Windows SDK." }
    return $tool.FullName
}

$useKeyLocker = -not [string]::IsNullOrWhiteSpace($KeypairAlias)

if ($useKeyLocker) {
    if (-not (Get-Command 'smctl' -ErrorAction SilentlyContinue)) {
        throw "smctl not found on PATH. Install the DigiCert client tools (CI: the 'Setup DigiCert signing tools' step)."
    }
    # smctl delegates .msi/.msix signing to signtool.exe and locates it via PATH only;
    # GitHub-hosted runners don't put the Windows SDK bin dir on PATH.
    if (-not (Get-Command 'signtool.exe' -ErrorAction SilentlyContinue)) {
        $env:PATH = (Split-Path -Parent (Resolve-SignTool)) + ';' + $env:PATH
    }
} else {
    if (-not (Test-Path -LiteralPath $SelfSignedPfx)) { throw "Self-signed .pfx not found: $SelfSignedPfx" }
    $signToolExe = Resolve-SignTool
}

function Invoke-KeyLockerSign([string]$file) {
    Write-Host "Signing $file (DigiCert KeyLocker, keypair '$KeypairAlias')"
    $output = & smctl sign --description "Obscura VPN Installer" --keypair-alias $KeypairAlias --input $file 2>&1
    $output | Write-Host
    # smctl can exit 0 on failure, so also check its output for the FAILED marker.
    if ($output -match 'FAILED') {
        # smctl's console output omits the failure reason; it lands in its log file.
        $smctlLog = Join-Path $env:USERPROFILE '.signingmanager\logs\smctl.log'
        if (Test-Path -LiteralPath $smctlLog) {
            Write-Host "--- smctl.log (last 40 lines) ---"
            Get-Content -LiteralPath $smctlLog -Tail 40 | Write-Host
        }
        throw "smctl sign failed for $file"
    }
}

function Invoke-SelfSignedSign([string]$file) {
    Write-Host "Signing $file (self-signed, $SelfSignedPfx)"
    $pw = @()
    if (-not [string]::IsNullOrEmpty($SelfSignedPfxPassword)) { $pw = @('/p', $SelfSignedPfxPassword) }
    & $signToolExe sign /d "Obscura VPN Installer (Self-Signed)" /fd SHA256 /f $SelfSignedPfx @pw $file
}

foreach ($file in $Files) {
    if (-not (Test-Path -LiteralPath $file)) { throw "File to sign not found: $file" }
    if ($useKeyLocker) { Invoke-KeyLockerSign $file } else { Invoke-SelfSignedSign $file }
}

Write-Host "Signing complete."
