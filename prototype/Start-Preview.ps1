$ErrorActionPreference = 'Stop'
$previewRoot = Join-Path $PSScriptRoot 'dist'
if (-not (Test-Path -LiteralPath (Join-Path $previewRoot 'index.html'))) {
  throw 'From the repository root, run npm ci and npm run build first.'
}
Write-Host 'GeoD Global design preview: http://127.0.0.1:4317/'
Write-Host 'This serves bundled local sample data. Press Ctrl+C to stop.'
python -m http.server 4317 --bind 127.0.0.1 --directory $previewRoot
