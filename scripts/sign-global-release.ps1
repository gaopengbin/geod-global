param(
    [Parameter(Mandatory=$true)][string]$Installer,
    [Parameter(Mandatory=$true)][string]$Version,
    [Parameter(Mandatory=$true)][string]$Url,
    [Parameter(Mandatory=$true)][string]$Notes,
    [Parameter(Mandatory=$true)][string]$Output
)
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSHOME 'Modules/Microsoft.PowerShell.Security/Microsoft.PowerShell.Security.psd1') -ErrorAction Stop
$keyFile = Join-Path $env:USERPROFILE '.codex/secrets/geod-global-release/signing-key.dpapi'
$protected = Get-Content -LiteralPath $keyFile -Raw | ConvertTo-SecureString
$pointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($protected)
try {
    $env:GEOD_GLOBAL_SIGNING_PRIVATE_KEY = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($pointer)
    & python -X utf8 (Join-Path $PSScriptRoot 'distribution-channel.py') sign-update --installer $Installer --version $Version --url $Url --notes $Notes --output $Output
    if ($LASTEXITCODE -ne 0) { throw 'Global release signing failed.' }
} finally {
    Remove-Item Env:GEOD_GLOBAL_SIGNING_PRIVATE_KEY -ErrorAction SilentlyContinue
    [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($pointer)
}
