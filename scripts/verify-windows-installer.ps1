[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Installer,
    [Parameter(Mandatory)][string]$Payload,
    [Parameter(Mandatory)][string]$Output,
    [string]$PreviousInstaller
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$repository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$verificationRoot = Join-Path $repository '.verification'
$outputRoot = [IO.Path]::GetFullPath($Output)
if (-not $outputRoot.StartsWith($verificationRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Installer acceptance output must be a new directory inside this repository .verification directory.'
}
if (Test-Path -LiteralPath $outputRoot) { throw 'Use a fresh acceptance directory.' }
$installerPath = (Resolve-Path -LiteralPath $Installer).Path
$payloadRoot = (Resolve-Path -LiteralPath $Payload).Path
$manifest = Get-Content -LiteralPath (Join-Path $payloadRoot 'release-manifest.json') -Encoding UTF8 -Raw | ConvertFrom-Json
$registrySubkey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\xyz.laogao.geod.global'
$registryPath = 'HKCU:\' + $registrySubkey
$shortcutPath = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\GeoD Global.lnk'
$dataRoot = Join-Path $env:LOCALAPPDATA 'xyz.laogao.geod.global'

function FileHash([string]$path) {
    $hash = [Security.Cryptography.SHA256]::Create()
    $stream = [IO.File]::OpenRead($path)
    try { return ([BitConverter]::ToString($hash.ComputeHash($stream))).Replace('-', '').ToLowerInvariant() }
    finally { $stream.Dispose(); $hash.Dispose() }
}
function DataSnapshot {
    $snapshot = @{}
    if (Test-Path -LiteralPath $dataRoot) {
        Get-ChildItem -LiteralPath $dataRoot -File -Recurse | ForEach-Object {
            $snapshot[$_.FullName.Substring($dataRoot.Length)] = FileHash $_.FullName
        }
    }
    return $snapshot
}
function AssertDataUnchanged($before) {
    $after = DataSnapshot
    if ($before.Count -ne $after.Count) { throw 'Application data file inventory changed.' }
    foreach ($name in $before.Keys) {
        if (-not $after.ContainsKey($name) -or $before[$name] -ne $after[$name]) { throw "Application data changed: $name" }
    }
}
function RunSilent([string]$program, [string[]]$arguments) {
    $process = Start-Process -FilePath $program -ArgumentList $arguments -WindowStyle Hidden -PassThru
    if (-not $process.WaitForExit(90000)) { throw 'Silent installer did not finish; no automatic force termination performed.' }
    if ($process.ExitCode -ne 0) { throw "Silent installer exited with $($process.ExitCode)" }
}

# Protect existing installation metadata and the exact shortcut bytes. The real
# installer is tested in an isolated install directory, then registration restored.
New-Item -ItemType Directory -Path $outputRoot | Out-Null
$installRoot = Join-Path $outputRoot 'installed application'
$registryBackup = @{}
$originalKey = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($registrySubkey)
$hadRegistration = $null -ne $originalKey
if ($originalKey) {
    try {
        foreach ($name in $originalKey.GetValueNames()) {
            $registryBackup[$name] = @{ value = $originalKey.GetValue($name, $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames); kind = $originalKey.GetValueKind($name) }
        }
    } finally { $originalKey.Close() }
}
$hadShortcut = Test-Path -LiteralPath $shortcutPath
if ($hadShortcut) { Copy-Item -LiteralPath $shortcutPath -Destination (Join-Path $outputRoot 'original-shortcut.lnk') }
$dataBefore = DataSnapshot
$checks = [Collections.Generic.List[string]]::new()
$canaryName = 'acceptance-user-file.txt'
$canaryValue = 'This unrecognized file must survive an upgrade and uninstall.'
$previousHash = $null
try {
    if ($PreviousInstaller) {
        $previousPath = (Resolve-Path -LiteralPath $PreviousInstaller).Path
        RunSilent $previousPath @('/S', "/D=$installRoot")
        $previousHash = FileHash (Join-Path $installRoot 'geod-global-desktop.exe')
        $checks.Add('Previous installer installed successfully in an isolated directory')
    }
    RunSilent $installerPath @('/S', "/D=$installRoot")
    if (-not (Test-Path -LiteralPath (Join-Path $installRoot 'geod-global-desktop.exe'))) { throw 'Installed desktop executable missing.' }
    Set-Content -LiteralPath (Join-Path $installRoot $canaryName) -Value $canaryValue -Encoding UTF8
    # Reinstall covers replacement of packaged files while retaining additional files.
    RunSilent $installerPath @('/S', "/D=$installRoot")
    foreach ($file in $manifest.files) {
        $installed = Join-Path $installRoot $file.path
        if (-not (Test-Path -LiteralPath $installed) -or (FileHash $installed) -ne $file.sha256) {
            throw "Installed payload hash mismatch: $($file.path)"
        }
    }
    if ((Get-Content -LiteralPath (Join-Path $installRoot $canaryName) -Encoding UTF8 -Raw).Trim() -ne $canaryValue) { throw 'Reinstall changed the user canary.' }
    $newHash = FileHash (Join-Path $installRoot 'geod-global-desktop.exe')
    if ($previousHash -and $previousHash -eq $newHash) { throw 'Previous and new binaries are identical; replacement was not exercised.' }
    $registration = Get-ItemProperty -LiteralPath $registryPath
    if ($registration.InstallLocation -ne $installRoot -or $registration.DisplayVersion -ne $manifest.version) { throw 'Installed registration mismatch.' }
    $checks.Add('All installed payload hashes match the release manifest')
    $checks.Add('Repeated installation preserves additional user files')
    AssertDataUnchanged $dataBefore
    $uninstaller = Join-Path $installRoot 'Uninstall GeoD Global.exe'
    RunSilent $uninstaller @('/S')
    # NSIS normally relaunches its uninstaller from a temporary copy. Wait for
    # completed effects rather than confusing the first process exit with success.
    $uninstallDeadline = [DateTime]::UtcNow.AddSeconds(60)
    while ((Test-Path -LiteralPath $uninstaller) -or (Test-Path -LiteralPath (Join-Path $installRoot 'geod-global-desktop.exe'))) {
        if ([DateTime]::UtcNow -gt $uninstallDeadline) { throw 'Uninstaller did not remove its application files.' }
        Start-Sleep -Milliseconds 200
    }
    foreach ($file in $manifest.files) {
        if (Test-Path -LiteralPath (Join-Path $installRoot $file.path)) { throw "Packaged file survived uninstall: $($file.path)" }
    }
    if ((Get-Content -LiteralPath (Join-Path $installRoot $canaryName) -Encoding UTF8 -Raw).Trim() -ne $canaryValue) { throw 'Uninstall removed or changed the user canary.' }
    AssertDataUnchanged $dataBefore
    $checks.Add('Uninstall removes packaged files and preserves additional user files')
    $checks.Add('All existing application data hashes remain unchanged')
} finally {
    # Restore exactly the pre-test values, including unknown extension values.
    if ($hadRegistration) {
        $restoreKey = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey($registrySubkey)
        try {
            foreach ($name in $restoreKey.GetValueNames()) {
                if (-not $registryBackup.ContainsKey($name)) { $restoreKey.DeleteValue($name) }
            }
            foreach ($name in $registryBackup.Keys) { $restoreKey.SetValue($name, $registryBackup[$name].value, $registryBackup[$name].kind) }
        } finally { $restoreKey.Close() }
    } elseif (Test-Path -LiteralPath $registryPath) {
        $current = Get-ItemProperty -LiteralPath $registryPath
        if ($current.InstallLocation -ne $installRoot) { throw 'Another installation owns the registration; cannot restore safely.' }
        [Microsoft.Win32.Registry]::CurrentUser.DeleteSubKey($registrySubkey, $false)
    }
    if ($hadShortcut) {
        Copy-Item -LiteralPath (Join-Path $outputRoot 'original-shortcut.lnk') -Destination $shortcutPath -Force
        if ((FileHash $shortcutPath) -ne (FileHash (Join-Path $outputRoot 'original-shortcut.lnk'))) { throw 'Shortcut restoration failed.' }
    } elseif (Test-Path -LiteralPath $shortcutPath) {
        Remove-Item -LiteralPath $shortcutPath
    }
}
$checks.Add('Original installation registration and shortcut restored')
$report = @{ installerSha256 = FileHash $installerPath; version = $manifest.version; previousDesktopSha256 = $previousHash;
    desktopSha256 = $newHash; checkedPayloadFiles = $manifest.files.Count; existingDataFiles = $dataBefore.Count;
    checks = $checks; scope = 'Silent per-user install, same-version replacement, reinstall and uninstall on the current Windows host; not clean-machine GUI acceptance' }
$report | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $outputRoot 'installer-verification.json') -Encoding UTF8
$report | ConvertTo-Json -Depth 5
