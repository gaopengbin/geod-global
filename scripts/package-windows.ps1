param(
    [ValidateSet('debug', 'release')][string]$Profile = 'release',
    [ValidateSet('nsis', 'none')][string]$Installer = 'nsis',
    [string]$Output,
    [switch]$BuildOnly,
    [switch]$PackageOnly
)
$ErrorActionPreference = 'Stop'
$packagingArguments = @((Join-Path $PSScriptRoot 'package-windows.py'), '--profile', $Profile, '--installer', $Installer)
if ($Output) { $packagingArguments += @('--output', $Output) }
if ($BuildOnly) { $packagingArguments += '--build-only' }
if ($PackageOnly) { $packagingArguments += '--package-only' }
& python @packagingArguments
if ($LASTEXITCODE -ne 0) { throw "Windows packaging failed with exit code $LASTEXITCODE" }
