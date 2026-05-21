param(
    [Parameter(Mandatory)][string]$Output,
    [Parameter(Mandatory, ValueFromRemainingArguments)][string[]]$Inputs
)
$ErrorActionPreference = "Stop"
if (-not (Test-Path -LiteralPath $Output)) { exit 1 }
$outTime = (Get-Item -LiteralPath $Output).LastWriteTimeUtc
foreach ($in in $Inputs) {
    if (-not (Test-Path -LiteralPath $in)) {
        Write-Error "input missing: $in"
        exit 1
    }
    if ((Get-Item -LiteralPath $in).LastWriteTimeUtc -gt $outTime) { exit 1 }
}
exit 0
