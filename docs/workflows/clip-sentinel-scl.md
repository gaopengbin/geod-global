# Download, inspect and clip a real Sentinel-2 SCL raster

This workflow uses the native GeoD Global runtime from an empty data directory.
It downloads one public Sentinel-2 scene-classification asset, validates a saved
recipe, and produces a real GeoTIFF plus a provenance JSON sidecar. The desktop,
loopback API and CLI use the same job manager and raster engine.

The example is `S2C_10SEG_20250707_0_L2A`, with a WGS84 rectangle around part of
San Francisco Bay: `[-122.55, 37.68, -122.32, 37.84]`. The downloaded asset is
scene classification (SCL), **not true-color imagery or a land-cover map**.
Its provider attribution is Earth Search / Element 84 and Copernicus Sentinel-2.

## 1. Build the CLI

From the repository root, with Rust installed:

```powershell
cargo build --locked -p geod-runtime
$clipRuntime = (Resolve-Path './target/debug/geod-runtime.exe').Path
$clipStore = Join-Path $PWD 'runtime-data'
```

Use a new, dedicated directory for this walkthrough. The runtime creates it. On
macOS/Linux the executable is `target/debug/geod-runtime` without `.exe` and the
same CLI flags apply. No Python, GDAL or external processing executable is needed
to run the product.

In PowerShell, this small helper parses the runtime's JSON output and stops when
a command fails. Diagnostics use stderr; stdout remains machine-readable JSON.

```powershell
function Invoke-GeoDJson {
    $commandOutput = & $clipRuntime @args
    if ($LASTEXITCODE -ne 0) { throw 'GeoD runtime command failed; inspect stderr.' }
    $commandOutput | ConvertFrom-Json
}
```

## 2. Download the pinned example scene

Review [`examples/sentinel-scl-download.json`](../../examples/sentinel-scl-download.json).
It contains the real scene ID, SCL URL and TIFF media type. The runtime accepts
only unsigned HTTPS URLs on its approved Sentinel bucket; recipes cannot insert
new network requests or arbitrary file paths.

```powershell
$sourceJob = Invoke-GeoDJson jobs download `
    --request './examples/sentinel-scl-download.json' --data-dir $clipStore
$sourceJob | Select-Object id, status, bytesDownloaded, sha256, outputPath
```

The command waits for transfer validation and the durable job record before
returning. Continue only with `status: succeeded`. A failed transfer returns a
nonzero exit code. Downloading needs network access; processing a completed source
is local.

```powershell
$sourcePreview = Invoke-GeoDJson jobs inspect --id $sourceJob.id --data-dir $clipStore
$sourcePreview | Select-Object width, height, crs, bounds, pixelSize, nodata
```

Inspection recalculates the source SHA-256, decodes its pixels and checks the
supported GeoTIFF metadata. It also returns full-resolution SCL class counts and
a PNG preview. Accepted sources are single-band UInt8 SCL rasters in a north-up
WGS84 UTM grid; unsupported data returns an explicit error.

## 3. Bind a recipe to this local source

The checked-in recipe is a reusable example with a source ID from a previous
workspace. Replace **both** its job ID and checksum with your newly completed
download. An imported recipe never auto-downloads a missing source.

```powershell
$clipRecipe = Get-Content './examples/sentinel-scl-clip.recipe.json' -Encoding UTF8 -Raw | ConvertFrom-Json
$clipRecipe.source.jobId = $sourceJob.id
$clipRecipe.source.sha256 = $sourceJob.sha256
$clipRecipePath = Join-Path $clipStore 'clip.recipe.json'
[IO.File]::WriteAllText(
    $clipRecipePath,
    ($clipRecipe | ConvertTo-Json -Depth 12),
    [Text.UTF8Encoding]::new($false)
)
```

Recipe files are UTF-8 JSON without a BOM and at most 8192 bytes. They have the
exact version `geod-raster-recipe/v1` and operation `clip`; unknown fields, versions,
operations, output formats and mismatched source checksums are rejected. A recipe
contains no absolute path, login token, executable command or user destination.

## 4. Review the actual window, then save

```powershell
$clipPlan = Invoke-GeoDJson recipes plan --recipe $clipRecipePath --data-dir $clipStore
$clipPlan.plan | Format-List
$savedRecipe = Invoke-GeoDJson recipes save --recipe $clipRecipePath --data-dir $clipStore
Invoke-GeoDJson recipes list --data-dir $clipStore
```

Planning writes no raster and saves no recipe. It reports the source grid, requested
bounds, projected bounds, actual output bounds, dimensions, pixel spacing, warnings
and the `[xOffset, yOffset, width, height]` source window. Saving validates the same
source and plan, then persists an independent recipe record in `recipes.json`.

The WGS84 request is converted to an enclosing rectangle in the source UTM CRS,
clipped to the source extent and expanded to whole pixels. The output uses the
original CRS and samples. It is not a geographic polygon mask, and pixels are not
reprojected or resampled. Review the plan's warnings before running. Set `crs` to
`source` to enter a rectangle directly in the source grid's metres.

For the example asset checked on 2026-09-22, the output window was
`[1980, 585, 1020, 895]`: 1,020 columns × 895 rows at 20 metres in EPSG:32610.

## 5. Run and inspect the real result

```powershell
$clipJob = Invoke-GeoDJson recipes run --recipe $clipRecipePath --data-dir $clipStore
$clipJob | Select-Object id, kind, status, parentId, sha256, outputPath, manifestPath
Invoke-GeoDJson jobs status --id $clipJob.id --data-dir $clipStore
$clipPreview = Invoke-GeoDJson jobs inspect --id $clipJob.id --data-dir $clipStore
$clipPreview | Select-Object width, height, crs, bounds, pixelSize, nodata
```

`recipes run` waits until the job finishes. Success requires the GeoTIFF, its
provenance sidecar and the persistent job record to have committed. The engine
decodes its output again and checks the copied samples and supported georeferencing.
The original downloaded source remains unchanged.

The files are under `assets/` with generated UUID names:

- `<job ID>.tif`: the real compressed GeoTIFF.
- `<job ID>.metadata.json`: recipe, source URL/attribution/pin, actual crop plan,
  relative TIFF filename, byte count and SHA-256.

Keep both files when sharing a result. The GeoTIFF contains its own CRS, transform
and nodata tags; the sidecar preserves the fuller processing history. This output
is a GeoTIFF, with no claim that it is a cloud optimized GeoTIFF.

Re-running the same recipe creates a different output job and filename. For the
same verified source and parameters, output TIFF bytes are deterministic:

```powershell
$secondClip = Invoke-GeoDJson recipes run --recipe $clipRecipePath --data-dir $clipStore
$secondClip.sha256 -eq $clipJob.sha256
```

The real example produced a 46,315-byte GeoTIFF with SHA-256
`af78deb054871d1f6b5c02de84f40fddbeac060ecd5d79781c305a7aa4063ecd`.
Its 912,900 samples, CRS, transform, resolution and nodata were independently
compared with the matching source window using Rasterio/GDAL. This establishes
the tested clip's pixel and metadata consistency, not geographic accuracy or
fitness for a particular scientific analysis.

## Use the running app adapter or recover a job

Only one manager can own a data directory. If the development service is already
running, use `--server` instead of `--data-dir`:

```powershell
Invoke-GeoDJson recipes plan --recipe $clipRecipePath --server http://127.0.0.1:4318
Invoke-GeoDJson recipes run --recipe $clipRecipePath --server http://127.0.0.1:4318
Invoke-GeoDJson jobs cancel --id JOB_UUID --server http://127.0.0.1:4318
Invoke-GeoDJson jobs retry --id JOB_UUID --server http://127.0.0.1:4318
```

Use a recipe whose pinned source is in that service's storage. The desktop itself
does not start this HTTP service by default; close the desktop before using direct
CLI access to its storage. Never delete or bypass its process lock.

Ctrl+C cancels a waiting direct CLI run and waits for worker cleanup. Jobs left
active by a killed process become `interrupted` on the next startup; retry is
explicit. Retrying a clip reuses its recipe and local source, cleans uncommitted
generated output, and reruns the clip. It does not download the source again.

## Optional independent verification

Python, Rasterio/GDAL and NumPy are needed only for this separate QA check, not
for GeoD execution. With those tools installed:

```powershell
python scripts/verify-raster-crop.py $clipStore $clipJob.id `
    --report (Join-Path $clipStore 'independent-verification.json')
```

The script computes the expected pixel window independently, checks every output
sample, reads GeoTIFF tags using GDAL, recalculates source/output checksums, and
verifies the provenance sidecar against the job record.
