# Download a pinned portable NSIS toolchain; no installation or machine settings.
# Official archive and SHA-256:
# https://sourceforge.net/projects/nsis/files/NSIS%203/3.11/nsis-3.11.zip/download
# 3.11 is an intentional fixed build dependency, not a claim about the latest release.
[CmdletBinding()]
param(
    [string]$Destination,
    # An optional CI cache is accepted only after the same complete hash check.
    [string]$ArchivePath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$version = '3.11'
$expectedSha256 = 'c7d27f780ddb6cffb4730138cd1591e841f4b7edb155856901cdf5f214394fa1'
$downloadUrl = 'https://downloads.sourceforge.net/project/nsis/NSIS%203/3.11/nsis-3.11.zip'
$sourcePage = 'https://sourceforge.net/projects/nsis/files/NSIS%203/3.11/nsis-3.11.zip/download'

if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
    throw 'NSIS setup requires Windows.'
}
if ([string]::IsNullOrWhiteSpace($Destination)) {
    if ([string]::IsNullOrWhiteSpace($env:RUNNER_TEMP)) {
        throw 'Pass -Destination outside GitHub Actions; RUNNER_TEMP is not set.'
    }
    $Destination = Join-Path $env:RUNNER_TEMP 'nsis'
}
$destinationPath = [IO.Path]::GetFullPath($Destination)
if ($destinationPath.IndexOfAny([char[]]"`r`n") -ge 0) {
    throw 'The destination must not contain line breaks.'
}
if (Test-Path -LiteralPath $destinationPath) {
    throw 'The NSIS destination already exists. Use a new empty path; existing tools are never trusted or overwritten.'
}
$parentPath = [IO.Path]::GetDirectoryName($destinationPath)
if ([string]::IsNullOrWhiteSpace($parentPath) -or -not [IO.Directory]::Exists($parentPath)) {
    throw 'The destination must have an existing parent directory.'
}
# Do not publish a tool path through a pre-existing junction or symbolic link.
for ($ancestor = [IO.DirectoryInfo]::new($parentPath); $null -ne $ancestor; $ancestor = $ancestor.Parent) {
    if (($ancestor.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw 'The destination parent chain must not contain reparse points.'
    }
}
if ($env:GITHUB_PATH -and -not [IO.File]::Exists($env:GITHUB_PATH)) {
    throw 'GITHUB_PATH must identify the existing runner environment file.'
}
if ($ArchivePath) {
    $cachedArchive = [IO.Path]::GetFullPath($ArchivePath)
    if (-not [IO.File]::Exists($cachedArchive)) { throw 'The cached NSIS archive does not exist.' }
} else {
    $curl = (Get-Command curl.exe -CommandType Application -ErrorAction Stop | Select-Object -First 1).Source
}

# A failed attempt is kept in this newly created directory for inspection. This
# script never recursively deletes, replaces, or silently reuses a destination.
[void][IO.Directory]::CreateDirectory($destinationPath)
$archive = Join-Path $destinationPath "nsis-$version.zip"
if ($ArchivePath) {
    [IO.File]::Copy($cachedArchive, $archive, $false)
} else {
    & $curl --fail --silent --show-error --location --max-redirs 5 --proto '=https' --proto-redir '=https' --connect-timeout 30 --max-time 180 --max-filesize 16777216 --retry 2 --output $archive $downloadUrl
    if ($LASTEXITCODE -ne 0) { throw "NSIS download failed with curl exit code $LASTEXITCODE." }
}
if (([IO.FileInfo]::new($archive)).Length -gt 16MB) { throw 'The NSIS archive exceeds the 16 MiB download limit.' }
$stream = [IO.File]::OpenRead($archive)
$sha256 = [Security.Cryptography.SHA256]::Create()
try {
    $actualSha256 = [BitConverter]::ToString($sha256.ComputeHash($stream)).Replace('-', '').ToLowerInvariant()
} finally {
    $stream.Dispose()
    $sha256.Dispose()
}
if ($actualSha256 -ne $expectedSha256) {
    throw "NSIS SHA-256 mismatch: expected $expectedSha256, received $actualSha256. Nothing was extracted or executed."
}

Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip = [IO.Compression.ZipFile]::OpenRead($archive)
$prefix = $destinationPath.TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
$names = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
$totalBytes = 0L
try {
    if ($zip.Entries.Count -gt 4096) { throw 'The NSIS archive contains too many entries.' }
    foreach ($entry in $zip.Entries) {
        $name = $entry.FullName
        if (-not $name.StartsWith("nsis-$version/", [StringComparison]::Ordinal) -or $name.Contains('\') -or $name.Contains(':')) {
            throw "Unexpected NSIS archive entry: $name"
        }
        $segments = $name.TrimEnd('/').Split('/')
        if ($segments -contains '..' -or $segments -contains '.' -or $segments -contains '') {
            throw "Unsafe NSIS archive entry: $name"
        }
        $target = [IO.Path]::GetFullPath((Join-Path $destinationPath $name))
        if (-not $target.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase) -or -not $names.Add($target.TrimEnd('\'))) {
            throw "Unsafe or duplicate NSIS archive path: $name"
        }
        $unixType = ($entry.ExternalAttributes -shr 16) -band 0xF000
        if ($unixType -eq 0xA000 -or ($entry.ExternalAttributes -band 0x400) -ne 0) {
            throw "NSIS archive links are not allowed: $name"
        }
        $totalBytes += $entry.Length
        if ($entry.Length -gt 32MB -or $totalBytes -gt 64MB) { throw 'NSIS archive expansion exceeds the size limit.' }
    }
} finally {
    $zip.Dispose()
}
[IO.Compression.ZipFile]::ExtractToDirectory($archive, $destinationPath)
$toolDirectory = Join-Path $destinationPath "nsis-$version"
$compiler = Join-Path $toolDirectory 'makensis.exe'
if (-not [IO.File]::Exists($compiler)) { throw 'The verified NSIS archive has no makensis.exe.' }
$reportedVersion = (& $compiler /VERSION | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or $reportedVersion -ne "v$version") {
    throw "The pinned NSIS compiler reported an unexpected version: $reportedVersion"
}
$record = [ordered]@{
    version = $version
    archiveSha256 = $actualSha256
    sourcePage = $sourcePage
    downloadUrl = $downloadUrl
    compiler = $compiler
    reportedVersion = $reportedVersion
    githubPathUpdated = [bool]$env:GITHUB_PATH
}
$encoding = [Text.UTF8Encoding]::new($false)
[IO.File]::WriteAllText((Join-Path $destinationPath 'nsis-source.json'), ($record | ConvertTo-Json), $encoding)
if ($env:GITHUB_PATH) {
    # GitHub applies this to subsequent job steps, not to the machine/user PATH.
    [IO.File]::AppendAllText($env:GITHUB_PATH, $toolDirectory + [Environment]::NewLine, $encoding)
}
$record | ConvertTo-Json
