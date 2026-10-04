# GeoD Global local runtime

`three-d discover` inspects a public HTTPS tileset/glTF/GLB entry. `save` uses
the inspected SHA-256 plus explicit rights to acquire its complete bounded
dependency graph. `open --file FILE` copies a local entry with its relative
dependencies or validates a GeoD export ZIP. `list`, `inspect --id UUID`,
`resource --request JSONFILE` and `export --id UUID --out FILE.zip` read verified
local content. `save`, `discover`, `open` and `resource` take JSON via `--request`.
Direct file open/export use `--data-dir`; the other operations also support
`--server`. As with other commands, do not open a directly owned store while its
desktop or service is running.

The matching HTTP routes are POST `/three-d/discover`, GET/POST
`/three-d/packages`, POST `/three-d/packages/import` (raw GeoD ZIP plus declared
rights/name query fields), GET `/three-d/packages/{id}`, GET
`/three-d/packages/{id}/resources/{resourceId}` and GET
`/three-d/packages/{id}/export`. They share the guarded loopback client boundary
and native core with seven Tauri commands. Reads return original bytes and
receipts; only the viewer/export scene view rewrites dependency URIs. ZIP import
retains originals, source resource origins and bounded rights history. This is
whole-source acquisition, not region selection or mesh clipping. Exact limits,
supported formats/extensions, actual public evidence and missing capabilities
are in [3D scene acceptance](../../docs/three-d-assets.md).

`tile-sources connect` discovers a public HTTPS PMTiles v3 MVT archive. `tile-packages extract` accepts `sourceId`, WGS84 `bounds`, `minZoom` and `maxZoom`; saves a standard tile-aligned PMTiles subset and immutable byte-range / version receipts. `tile-packages open --file FILE --data-dir DIR` verifies and copies a complete local PMTiles or MBTiles archive without changing its bytes. `list`, `inspect`, `tile --request JSONFILE` and explicit-path ZIP `export` read managed local bytes. MBTiles uses bundled read-only SQLite, ordinary tables / compatible views, TMS-to-XYZ inventory, gzip MVT or PNG / JPEG; original database and metadata remain intact. The Tauri commands and protected loopback routes share this core. Limits: PMTiles none/gzip compression, 512 candidate / addressed tiles, 128 MiB, zoom ≤24; synchronous acquisition, no resume. See [public PMTiles acceptance](../../docs/pmtiles-offline.md), [local PMTiles](../../docs/pmtiles-local.md) and [local MBTiles](../../docs/mbtiles-local.md).

`map-services connect` accepts `protocol: "WMTS"` with `wmtsDocument: true` for a direct Capabilities XML address. Compatible advertised REST ResourceURL templates retain selected time, style, opaque matrix identifiers, exact source tile receipts and native pixel coordinates. Legacy KVP records remain readable. See [WMTS REST verification and limits](../../docs/wmts-rest-map-images.md).

`map-services connect` also accepts `protocol: "XYZ"` / `"TMS"` with explicit `tileConfig`. The native core validates public path templates, fixed global Web Mercator grid, tile size and URL zoom offset; saves exact source tiles and an unresampled pixel window; and retains the logical matrix, row direction and native georeferencing. Connecting saves user configuration; it does not invent capabilities. See [XYZ/TMS evidence and limits](../../docs/xyz-tms-map-images.md).


`map-services connect` accepts `protocol: "ArcGIS"` for public HTTPS MapServer / ImageServer roots. It retains exact service / layer JSON and obtains export JSON before downloading the generated PNG. Returned extents determine the saved WGS84 grid; a persisted per-request UUID avoids expired cached hrefs. Same-origin generated output URLs, raw response receipts, offline read checks and ZIP sidecars use the existing map-image core. See [actual ArcGIS rendering acceptance](../../docs/arcgis-map-images.md).

This crate is independent of the domestic application. The Tauri application and
the loopback development adapter use the same `JobManager` implementation.

```sh
cargo test -p geod-runtime
cargo run -p geod-runtime -- serve --data-dir .verification/runtime-data --port 4318
```

Direct CLI commands require a storage directory; `--server` commands reuse an
already running loopback adapter. Desktop code supplies its own application data
directory. A process lock prevents two runtimes from writing the
same storage directory. Raster output filenames use generated UUIDs under `assets/`;
prepared delivery ZIPs use generated job filenames under `exports/`. Requests
cannot supply a destination path.

## Local vector files

`vectors open --file FILE --data-dir DIR` registers a native file by reference;
`--mode managed` explicitly copies it. `vectors list|inspect|forget` also support
the loopback server adapter. `vectors export --id UUID --out FILE --data-dir DIR`
saves verified GeoJSON and rejects registered source destinations. Optional
`--format original` exports the verified complete original; GeoPackage retains
its SQLite bytes. Shapefile retains the exact input ZIP or an explicitly identified
deterministic bundle of unchanged SHP companion files. Opening or saving native paths
is direct-mode only: the browser adapter imports bounded JSON, raw GeoPackage or ZIP
bytes and has no arbitrary file path routes. Registrations persist,
and inspections recheck full source hashes. Forgetting never deletes an original.
The desktop uses its native file chooser for these operations.

Accepted formats are RFC 7946 WGS84 GeoJSON, complete embedded-geometry
Overpass JSON with node, way, area-relation and complete nested relation support,
and standalone GeoPackage 1.2–1.4 ordinary feature tables. GeoPackage preserves
all original bytes and typed provenance, reads core WKB geometry, retains Z / M
without vertical transformation, and converts horizontal coordinates with one
recorded datum operation per layer. It reports coordinates outside that operation's
declared area; it does not choose regional methods per coordinate or download grids.
See [GeoPackage scope and independent checks](../../docs/geopackage-local.md).
Shapefile SHP/ZIP supports bounded core XY/Z/M point, line, polygon and multipoint
layers with explicit PRJ, original DBF fields/encoding and deleted/null records.
Exact numeric DBF lexemes export as strings; deleted records stay in exports and
are omitted from the map. Native SHP opening collects its supported same-folder
companions; ZIP original exports are byte-identical. Polygon work is bounded and
topology is never repaired. See [Shapefile scope and evidence](../../docs/shapefile-local.md).
Files are limited to 20 MiB, 50,000 features and 500,000 coordinates. Existing
OSM registrations retain their legacy conversion and checksums. New registrations
record conversion version 2 and retain node/member references and OSM metadata.
See [real-file verification and format boundaries](../../docs/vector-local-data.md).

`feature-services connect` accepts protocol `Overpass` with an explicit public
HTTPS interpreter URL. The app has no default public instance. Five fixed
presets query bounded areas (100 square kilometres, at most one degree per side),
retrieve complete recursive dependencies and persist the exact response bytes.
Only selected roots become features. Both typed completion counters and the raw
SHA-256 are checked on reopening; removing a connection does not remove its
saved files. See [online OSM behavior and limits](../../docs/osm-overpass.md).

The same feature-service commands accept `protocol: "WFS2"` for a public WFS
2.0 endpoint. Queries optionally select `responseFormat: "gml32" | "geojson"`;
GML 3.2 is preferred when advertised. The managed `wfs-snapshot` archive retains
both schemas, numeric hit responses and two complete reads of all matching
features. Per-document hashes and a structural schema comparison are checked
again before inspection/export. GeoJSON export preserves first-read feature IDs,
all supported scalar properties and complete 2D geometry in longitude/latitude.
The archive hash is distinct from the derived GeoJSON hash. Legacy OGC, ArcGIS,
Overpass and direct-file records require no migration. See [WFS behavior and
acceptance](../../docs/wfs-features.md) for limits and unsupported feature models.

## Transfer behavior

Custom public raster sources use a separate immutable STAC snapshot plus the
original asset key, never the ordinary provider URL endpoint. `stac connect`,
`search`, `snapshot`, `project`, `download`, `inspect`, and `pixel` support both
direct storage and the loopback adapter. Snapshot IDs are SHA-256; job and
connection IDs are UUIDs. API GET / POST Item Search, standalone Items and direct
GeoTIFF URLs retain source documents and transfer identity. Pagination preserves
the method and JSON body; refreshed equivalent selections reuse the project's
original pin. Legacy GET registries reopen without rewriting saved bytes. See
[POST search acceptance](../../docs/stac-post-search.md). Generic inspection
reads raw samples without inferring sensor calibration from asset names. Reuse
checks the entire local SHA-256 and queues replacements for missing or changed
originals. See [custom raster contracts and limits](../../docs/custom-stac.md).

Sentinel-1 IW RTC inspection reads managed Float32 gamma0 originals on their
own 10 m WGS84 UTM Area grid. It rechecks the whole original checksum before
bounded TIFF decoding, uses checked embedded overviews for display, and returns
full-resolution linear gamma0 and optional dB for pixel queries. Zero remains a
valid linear value without finite dB; -32768 is NoData. The shared persistent
thumbnail cache also supports RTC COGs and project outputs. RTC crop and aligned
mosaic preserve Float32 bits, polarization, valid zero and -32768 NoData; the
newest valid scene wins overlaps without averaging. Two complete actual VV
originals and three outputs have independent GDAL/PROJ/GEOS verification of all
6,685,308 output values. Other polarizations, reprojection, filtering and raw
GRD/SLC processing remain pending. See [radar inspection](../../docs/radar-inspection.md)
and [project processing acceptance](../../docs/radar-processing.md).

- The ordinary provider endpoint accepts only reviewed unsigned source URLs: Earth Search S2, Planetary
  Computer S2 / Landsat C2 bands / NAIP RGB+NIR COGs, Copernicus DEM GLO-30 Public / GLO-90,
  NASA HLS L30 v2.0 bands, SRTMGL1 v003 HGT ZIPs, VIIRS 09A1 v002 HDF5 originals,
  MODIS Terra/Aqua 09A1 v061 converted COG bands, Sentinel-1 IW RTC polarization COGs,
  and Copernicus S2 L2A
  SAFE products. Paths and asset keys are bound to the product; PC, NASA and CDSE
  workers also check the current official item. Only narrowly reviewed provider
  redirects are followed. Native authorization and temporary signatures are never
  exposed by the API or persisted in jobs. See [provider boundaries](../../docs/providers.md).
- At most two transfers run concurrently; up to 64 active or queued jobs are
  accepted. Ordinary raster and VIIRS HDF5 transfers are limited to 512 MiB;
  reviewed Sentinel-1 RTC and NAIP RGB/NIR COGs, and SAFE ZIP products to 4 GiB;
  SRTM HGT ZIPs to 64 MiB.
  Transfers have a 20-second connection timeout and a 45-second
  read inactivity timeout, without a total duration deadline for healthy streams.
  Catalogue/account requests retain their separate bounded deadlines.
  Reviewed ordinary public original assets support conditional byte-range
  recovery on explicit retry. The saved prefix is rehashed and bound to the job,
  source, asset, strong ETag and total size. Only a matching 206 response resumes;
  a changed version or rejected range restarts without combining versions.
  Protected NASA/CDSE downloads, custom STAC and generated WCS responses still
  restart from byte zero; their separate adapter contracts are unchanged.
  See [transfer policy and verification](../../docs/source-transfer.md).
- NAIP retains the exact official item identity for the reviewed 0.3/0.6/1 m
  filename variants. Legacy `1` tokens mean 1 m. The historical four-channel
  TIFF alpha tag is treated as NIR only for that catalogue-bound variant;
  original bytes are unchanged and display opacity is independent. The
  download, processing-source and thumbnail limits share the 4 GiB policy.
  See [real-file evidence and remaining cases](../../docs/naip-resolution-variants.md).
- Data streams into a `.part` file while SHA-256 and byte count are calculated.
  Success requires a recognized TIFF/JPEG/ZIP/HDF5 signature and an exact Content-Length
  match when the server supplies it. The file is synced and atomically renamed
  before success is recorded.
- These are **file signature and transfer integrity checks**, not scientific
  raster validation, source authenticity checks, or full TIFF/JPEG decoding.
  VIIRS downloads now check embedded v002 identity, period, sinusoidal grid and
  M5/M4/M3 calibration and decode all original Int16 samples before commit.
  This reader is validated with independent synthetic HDF5 fixtures, not
  authenticated production products. Managed M5/M4/M3 GeoTIFF preparation,
  original pixels, local RGB, persistent previews and project area processing
  are available; nine prepared synthetic bands and a single-scene clip were
  independently verified. Production products, VIIRS multi-scene acceptance
  and other science/QA layers remain pending. See
  [VIIRS processing](../../docs/viirs-processing.md).
  SAFE products additionally require the selected product directory, metadata and
  TCI/SCL entries in a complete, bounded ZIP directory. Download validation does
  not decode JP2 or verify the provider checksum. A separate local preparation
  task validates ZIP CRC, XML geometry and original JP2 samples; see
  [SAFE processing](../../docs/safe-processing.md). Catalogue size is an
  estimate, not a substituted HTTP transfer length. Available disk space is checked.
- Cancellation stops the transfer and removes its saved partial and checkpoint.
  Explicit retry uses the same job ID. Eligible partials may be recovered after
  an interruption or clean shutdown; resumed bytes are reported only after
  response validation. An authorization denial fails without bypassing access
  checks or automatically retrying. Full length, signature, SHA-256 and applicable
  source validation remain required before any partial becomes a usable asset.
- Records persist in `jobs.json` through a synced temporary file and rename.
  Jobs left queued/running after a process exit become `interrupted` when the
  runtime next opens. They do not resume automatically.
- Source URL, item ID, asset key, media type, timestamps, attempt count, checksum,
  output path and validation scope stay with each record.

## Local development API

The service binds exclusively to `127.0.0.1`. It accepts the exact browser origin
`http://127.0.0.1:4317`, rejects other origins and non-loopback Host headers, and
requires `X-GeoD-Client: geod-global` for POST requests. Its CORS allowlist contains
only that origin, GET/POST/OPTIONS, Content-Type and X-GeoD-Client. Requests without
an Origin are permitted for local command-line clients; this service is not a
network-facing authentication boundary.

| Method | Route | Result |
| --- | --- | --- |
| GET | `/health` | Runtime version, storage root, validation scope and size limit |
| GET | `/diagnostics` | Allowlisted `geod-support-diagnostics/v1` aggregate report, without paths or user text |
| GET | `/accounts` | Only redacted account status; credential mutations require native desktop ACL |
| POST | `/providers/copernicus/products` | Public original-product metadata for 1–32 distinct, validated S2 L2A scene IDs; no credential access |
| GET | `/jobs` | Array of persisted jobs, newest first |
| GET | `/jobs/{id}` | One persisted job plus `settled`, or 404 |
| POST | `/jobs` | `202` and a queued job |
| POST | `/jobs/{id}/cancel` | Updated job |
| POST | `/jobs/{id}/retry` | Queued retry, or error if still settling |
| GET | `/jobs/{id}/raster` | Verified RGB / SCL / Landsat C2 / HLS L30 / NAIP / DEM geometry and PNG; product-specific display metadata |
| GET | `/jobs/{id}/pixel?x=...&y=...` | Full-resolution original SCL, RGB, UInt16 / Int16 DN and calibrated reflectance at finite source-CRS coordinates |
| GET | `/jobs/{id}/file` | Revalidated derived GeoTIFF as an attachment |
| GET | `/jobs/{id}/metadata` | Revalidated project-mosaic provenance JSON as an attachment |
| GET | `/projects` | Persisted multi-scene projects |
| POST | `/projects` | Save selected scenes, area and optional polygon |
| POST | `/projects/{id}/downloads` | Queue or reuse `scl` / `visual`, reviewed Landsat / HLS `red` / `green` / `blue` original bands, or CDSE `product` SAFE ZIP; optional `itemIds` restricts downloads to selected project scenes |
| POST | `/projects/{id}/scenes` | Append validated scenes without replacing existing pinned sources, project name or clipping area |
| POST | `/projects/{id}/rasters` | Prepare or reuse `visual` / `scl` GeoTIFFs from completed, checksum-pinned SAFE originals; no account is needed for this local operation |
| POST | `/projects/{id}/mosaics` | Queue a pinned, pixel-aligned mosaic and area clip for `scl`, `visual`, `red`, `green` or `blue` |
| POST | `/jobs/{id}/package` | Verify and prepare a derived-output ZIP; return its metadata as JSON |
| GET | `/jobs/{id}/package` | Read and revalidate an already prepared ZIP; return its bytes as an attachment |
| GET | `/recipes` | Saved recipes, newest first |
| POST | `/recipes/plan` | `{ recipe, plan }`; validates and computes an actual pixel window without writing files |
| POST | `/recipes` | `201` and `{ id, recipe, createdAt, updatedAt }`; validates the source and plan before saving |
| POST | `/recipes/run` | `202` and a queued `raster_clip` job; execution freshly verifies the pinned source |

Create requests use `{ "itemId", "assetKey", "href", "mediaType", "title" }`;
`title` is optional. JSON responses use camelCase and status values
`queued`, `running`, `succeeded`, `failed`, `cancelled`, `interrupted`.
JSON request bodies are limited to 2 MiB so saved administrative polygons fit. CLI recipe files are limited to 512000 bytes; CLI download request files remain limited to 8192 bytes. Pixel query arguments contain only
`x` and `y`; unknown fields are rejected. The package POST does not accept an
output path or require an input document; it uses the job identified in the route
and still requires the mutation header. Runtime validation/busy failures return
HTTP 400 with an `error` string, distinct from a completed job or package.

The deterministic tests use a local TCP fixture **only under `cfg(test)`**. The
production API has no flag, environment variable or localhost URL override.

## Multi-scene projects and mosaics

Projects persist separately in `projects.json` and contain 1–32 catalog scenes,
their approved source asset URLs and band metadata, WGS84 search bounds and an optional
WGS84 polygon. Each asset type is downloaded into ordinary jobs, with completed
or active matching source jobs reused. A `raster_mosaic` job pins every source
job ID and SHA-256 and produces its own GeoTIFF plus `.metadata.json` manifest.
Output is streamed through bounded strips with a conservative disk-space check.
Source files remain unchanged; the previous 8-million-pixel total limit is removed. The newest non-NoData pixels win, and polygon masking uses output
pixel centres. The output preserves its UTM grid, source band type and pixel
spacing; different UTM zones or unaligned grids fail explicitly. SCL is
single-band UInt8; true-color TCI is three-band UInt8. Landsat C2 L2 UInt16 and
HLS L30 v2 Int16 retain original DN, product NoData and versioned scale/offset
in the GeoTIFF and optional plan `calibration`. Mixed products/calibration fail.
No reprojection or
resampling is performed. The raw result can be downloaded by the browser or
revealed in the desktop file explorer. The existing ZIP bundle contract still
covers single-source `raster_clip` outputs only.

## Native reflectance band inspection

Completed managed Landsat Collection 2 Level-2 `red` / `green` / `blue` UInt16
and HLS L30 v2.0 Int16 files use a separate typed reader. It binds the original
product, source host, identity and channel before applying the versioned official
scale/offset; TIFF sample format, 30 m spacing and NoData must match that product.
It verifies the whole source snapshot (up to 512 MiB), then decodes bounded chunks
for a nearest-neighbour grayscale preview or exact full-resolution pixel query.
PixelIsPoint centres are translated into outer pixel edges; PixelIsArea is
retained. Other CRS/grid restrictions remain explicit.

Inspection has an optional `reflectance` object: product, band, scale, offset,
pixel interpretation, display DN range, total display samples and valid samples.
The range is the sampled 2–98 percentiles after excluding NoData, not whole-scene
statistics. Original `value` and `nodata` support signed 16-bit values; a valid
pixel has `reflectance`, which is neither clamped nor read from the display PNG.
NoData omits that value. Existing RGB/SCL responses omit the new optional fields.
Local thumbnails use the same bounded persistent cache. Project processing
clips/mosaics these typed original bands with no scientific value rescaling.
Output inspection binds its recorded calibration, channel and geometry.
No QA mask or RGB band composition is implemented.
See [inspection evidence](../../docs/reflectance-inspection.md) and
[typed processing evidence and limits](../../docs/reflectance-processing.md).

## Native SCL raster inspection

`JobManager::inspect_raster(id)` and desktop `inspect_raster { id }` inspect a
completed SCL GeoTIFF, with no external processing executable. The pinned pure
Rust `tiff` 0.11.3 and `png` 0.18.1 libraries decode the image and encode its preview.
The original asset remains unchanged.

Each request resolves the job's UUID filename inside its canonical managed
`assets` directory, checks its recorded byte count and recalculates SHA-256.
The exact verified bytes are then decoded, so cached metadata never bypasses a
fresh integrity check. A shared semaphore admits one blocking raster inspection,
pixel read, plan, clip or package operation at a time; interactive requests receive
a busy error while another operation owns it, and clip jobs wait in the queue. The decoder has explicit allocation limits,
and cooperative deadline checks run during reading and pixel processing.

Supported files are single-band unsigned 8-bit, grayscale, top-left, north-up
Sentinel-2 SCL rasters, with values 0–11. The local file limit is 128 MiB, the
decoded raster limit is 64 × 1024 × 1024 pixels, and each edge is at most 16,384.
JPEG, RGB, palette-encoded TIFF, other sample types, rotated coordinates and
unsupported compression return explicit errors. Compressed TIFF support currently
includes deflate and LZW.

CRS, scale, tiepoint/transform and nodata come from GeoTIFF tags. Inspection
requires projected PixelIsArea coordinates and a WGS84 UTM EPSG code in
32601–32660 or 32701–32760. It supports either scale plus one tiepoint, or an
unrotated transformation matrix. Conflicting or missing tags are errors. Bounds
are outer pixel edges in `[minX, minY, maxX, maxY]` order, in the CRS's metres;
`pixelSize` is positive `[x, y]` spacing. `nodata` is `null` when its tag is absent.
The supported explicit SCL nodata value is zero.

The PNG preserves aspect ratio with maximum edge 768 and never enlarges a small
raster. Nearest sampling uses `floor(x * width / previewWidth)` and the equivalent
y formula. Counts include every full-resolution pixel, including the no-data
class. Explicit nodata pixels are transparent; other classes are opaque.

Class meanings follow the ESA Sentinel-2 scene classification convention. Display
colors reference the [Sentinel Hub SCL legend](https://custom-scripts.sentinel-hub.com/custom-scripts/sentinel-2/scene-classification/)
(CC BY-SA 4.0, Sentinel Hub; code here independently applies its factual class/color
mapping), which links the ESA Level-2A algorithm documentation. This preview is
scene classification, not a land-cover analysis or a true-color scene image.
Successful inspection establishes readable samples and consistent supported
metadata; it does not establish geolocation accuracy or scientific suitability.

Library documentation: [TIFF decoder and allocation limits](https://docs.rs/tiff/0.11.3/tiff/decoder/index.html),
[PNG encoder](https://docs.rs/png/0.18.1/png/struct.Encoder.html).

### Source pixel queries

`JobManager::sample_raster(id, x, y)`, desktop `sample_raster { id, x, y }` and
GET `/jobs/{id}/pixel?x=...&y=...` read the supported file at full resolution.
Coordinates are in the inspected raster's UTM metres, not longitude/latitude or
preview-image pixels. Each call rereads and verifies the original file through
the same bounded decoder; a previous inspection does not bypass checksum checks.

The response contains `jobId`, `sha256`, `crs`, requested `coordinate: [x, y]`,
zero-based `pixel: [column, row]`, `center: [x, y]` for that pixel's centre,
`value`, `label`, `color` and `isNoData`. Column is
`floor((x - minX) / pixelWidth)`; row is
`floor((maxY - y) / pixelHeight)`. The left/top outer edges are included and the
right/bottom outer edges are excluded. Outside and non-finite coordinates fail;
they are not clamped to a nearby valid pixel. `isNoData` means the value matches
the file's explicit nodata tag, not merely that the SCL class label is “No data”.

## Executable raster recipes

The executable schema is `geod-raster-recipe/v1`. It is distinct from earlier
design/export simulations. Every object rejects unknown fields. A recipe contains
only a name, a local source job and SHA-256, a clip operation, and a GeoTIFF format:

```json
{
  "schemaVersion": "geod-raster-recipe/v1",
  "name": "Local SCL clip",
  "source": {
    "jobId": "00000000-0000-4000-8000-000000000000",
    "sha256": "0000000000000000000000000000000000000000000000000000000000000000"
  },
  "operation": {
    "type": "clip",
    "crs": "EPSG:4326",
    "bounds": [-122.45, 37.70, -122.35, 37.80]
  },
  "output": { "format": "GeoTIFF" }
}
```

The UUID and hash above are placeholders: replace them with a completed local
SCL job from `jobs list`. Importing JSON does not download or discover its source.
The same source job ID and lowercase SHA-256 must exist in the selected storage.
Names allow 1–120 Unicode characters, must contain a non-whitespace character, and
must not contain control characters. Source IDs use canonical lowercase hyphenated
UUIDs; hashes use exactly 64 lowercase hexadecimal characters.

`operation.crs` is exactly `source` or `EPSG:4326`. Bounds are finite increasing
`[west, south, east, north]`; source coordinates use the source UTM metres. WGS84
coordinates are limited to longitude −180…180, UTM latitude −80…84 and a longitude
span at most 180 degrees. Antimeridian-crossing requests are rejected. WGS84 edges
are densified into a source-CRS envelope; the output remains in the original UTM
CRS. This is a rectangular pixel-window clip, without resampling, reprojection of
pixel values, or polygon masking. Requests are intersected with the source extent
and expanded outward to complete pixels. The plan reports requested, projected,
source and actual bounds, `[xOffset, yOffset, width, height]`, pixel spacing and
explicit warnings. A request with no intersection fails.

The engine reuses inspection's size/format/CRS checks, verifies the exact pinned
source bytes, and copies the original UInt8 class values. It writes a compressed
GeoTIFF with CRS, pixel scale, origin and nodata tags, then decodes and checks the
output before publishing it. It does not claim to write a cloud optimized GeoTIFF.
Original sources are never modified; completed derived SCL jobs may themselves be
used as pinned inputs to another recipe.

Recipe records are saved atomically in `recipes.json`; every save creates an
independent immutable record. Runs create separate UUID jobs and retain the recipe,
parent job ID, original source URL and actual crop plan. The output GeoTIFF and an
`assets/<job UUID>.metadata.json` provenance sidecar must both be complete before
the job can become `succeeded`. The sidecar uses `geod-raster-artifact/v1` and
contains the output's relative filename, size and SHA-256, source attribution and
pin, recipe and crop plan. Keep the TIFF and sidecar together when copying a result.

Cancellation is cooperative during reading, decoding, copying and writing; the
final job commit checks cancellation again. Failed or cancelled runs expose no
completed output and clean their generated artifacts. Retry uses the same job ID
and recipe, never re-downloads a clip's source, and starts the operation again.
An interrupted process does not silently resume: reopening storage marks active
jobs `interrupted` and removes their uncommitted generated clip files. Existing
download records without `kind` remain compatible and default to `download`.

The equivalent desktop commands are `list_recipes`, `plan_recipe { recipe }`,
`save_recipe { recipe }`, and `run_recipe { recipe }`. Clips use the existing
`cancel_job`, `retry_job`, `inspect_raster` and `reveal_job` commands.

## Verified delivery packages

`JobManager::prepare_artifact(id)`, desktop `prepare_artifact { id }` and POST
`/jobs/{id}/package` accept only successful managed `raster_clip` jobs. The runtime
checks the recipe/source record relationship, exact managed TIFF and sidecar
paths, TIFF byte count and SHA-256, and the sidecar's agreement with committed
source, recipe and crop records. TIFF input is limited to 32 MiB and the sidecar
to 64 KiB. This operation packages a completed result; it does not run a recipe or
include/revalidate the source raster's original bytes.

The ZIP contains `<job UUID>.tif`, `<job UUID>.metadata.json`, `recipe.json`,
`README.txt` and `checksums.sha256`. It retains exact TIFF/sidecar bytes, uses
relative entry names, and includes source attribution plus the user-defined
recipe name and spatial bounds. It contains no absolute local paths. Review its
contents before sharing; the pinned recipe is not automatically portable to
another store and does not download its missing source.

Preparation writes a synced temporary file and publishes it without overwriting
an existing file. The result JSON has `jobId`, `filename`, `path`, `bytes`,
`sha256` and `files`. Identical repeat preparations reuse the verified existing
package; a changed existing ZIP is rejected, never silently overwritten.

GET `/jobs/{id}/package` does not create a package. It requires a prior successful
preparation, rechecks the current TIFF/sidecar against the records, reconstructs
the expected package and compares the prepared ZIP bytes before returning them.
Its headers are `Content-Type: application/zip`, an attachment filename,
`Cache-Control: no-store`, and an ETag containing the package SHA-256. Desktop
`reveal_artifact { id }` performs the same verification before showing its folder.
These are runtime/API capabilities; actual browser, desktop and download
acceptance is recorded separately in [the workspace and delivery record](../../GeoD-Global-Spec/12-Workspace-Agent-and-Distribution.md).

## Local support diagnostics

GET `/diagnostics` and desktop `diagnostics` return
`geod-support-diagnostics/v1`: runtime/version, OS/architecture, supported
capabilities, limits, counts for each job status, saved recipe count and explicit
privacy flags. The report excludes file paths, coordinates, source URLs and user
text, and generating it uploads nothing. It is not a raw dump of `/health`, jobs,
recipes or errors; those operational responses may contain local paths and source
metadata. The application shows the report for the user to review and copy.

## CLI

Build with `cargo build -p geod-runtime`, then use `target/debug/geod-runtime`
(`.exe` on Windows). `--help` returns a JSON command summary. Every command emits
machine-readable JSON on stdout; errors are JSON on stderr with exit status 1.
The service startup line goes to stderr. Downloads, retries and recipe runs wait
until a terminal result and return success only after durable output completion.
Ctrl+C requests cancellation and waits for worker cleanup in either mode.
`jobs cancel` also waits for cleanup, then exits successfully when the cancellation
request has settled. A cancelled `run`/`download`/`retry` exits with status 1.

```sh
geod-runtime jobs download --request download.json --data-dir ./runtime-data
geod-runtime jobs list --data-dir ./runtime-data
geod-runtime jobs status --id SOURCE_JOB_UUID --data-dir ./runtime-data
geod-runtime jobs inspect --id SOURCE_JOB_UUID --data-dir ./runtime-data
geod-runtime recipes plan --recipe clip.json --data-dir ./runtime-data
geod-runtime recipes save --recipe clip.json --data-dir ./runtime-data
geod-runtime recipes list --data-dir ./runtime-data
geod-runtime recipes run --recipe clip.json --data-dir ./runtime-data
```

`download.json` is the same strict `CreateJobRequest` body as POST `/jobs`; use
`assetKey: "scl"` and the selected scene's actual SCL URL and media type. The normal
HTTPS bucket, scene identity, file extension and transfer guards always apply.
`clip.json` contains the raw recipe object, not the saved record wrapper.

While the development service owns storage, use its adapter instead of opening a
second manager. The client allows only HTTP `127.0.0.1` origins and follows no
redirects, sends the required mutation header, and polls `/jobs/{id}` to completion.
This response adds `settled`, read together with the job under the same lock.
The CLI waits for both a terminal state and `settled: true`; an early `cancelled`
record alone cannot make it exit while the worker still owns temporary output:

```sh
geod-runtime recipes run --recipe clip.json --server http://127.0.0.1:4318
geod-runtime jobs cancel --id JOB_UUID --server http://127.0.0.1:4318
geod-runtime jobs retry --id JOB_UUID --server http://127.0.0.1:4318
```

The desktop does not expose a loopback service by default. Close it before using
direct CLI access to its runtime storage; do not delete or bypass `runtime.lock`.
Use a separate data directory for independent CLI processing.

## Public WCS coverage subsets

`wcs connect`, `describe`, `plan`, `project` and `download` use the same native
manager as the desktop. Requests use JSON files through `--request`; saved
definitions and plans are read with `description --id SHA256` and
`snapshot --id SHA256`. `inspect` and `pixel` operate on completed local jobs.
Use either `--data-dir` or an already-running loopback `--server`.

Supported WCS 2.0.1 KVP requests retain the original capabilities and coverage
description, calculate a native-grid rectangular subset, and validate the returned
GeoTIFF before committing it. This is a server-generated subset, not a provider
source archive. Units, nil declarations, TIFF tags and raw samples remain separate;
no calibration is inferred. The `geod_wcs_*` MCP tools expose the same saved
connections, native plans, project downloads, inspection and raw pixel values.
See [coverage scope and verification](../../docs/wcs-coverages.md).

## MCP stdio adapter

`serve-mcp` uses the official `rmcp` SDK, pinned to 3.4.0, with the same Rust
manager and strict job/recipe inputs. Choose exactly one ownership mode:

```sh
geod-runtime serve-mcp --server http://127.0.0.1:4318
geod-runtime serve-mcp --data-dir ./agent-runtime-data
```

Fifteen read/inspection/preflight tools are exposed by default. The explicit startup
flag `--allow-write` enables eleven additional source, plan, project, download,
recipe and task mutation tools; arguments cannot enable them. WCS discovery and
download tools contact the user-selected public service, while saved catalog,
plan and file reads are local. This is newline-delimited JSON-RPC on
stdio, not a public or Streamable HTTP MCP endpoint. Stdout is protocol-only and
diagnostics use stderr; launch the binary directly without a wrapper that writes
status text to stdout.

In `--server` mode the already-running loopback service owns jobs, so disconnecting
MCP does not stop them. In `--data-dir` mode MCP owns the exclusive store and normal
EOF/Ctrl-C drains started calls, cancels active jobs and waits for worker cleanup.
Opening a direct store still performs normal storage initialization and recovery;
default read-only tool access is not a zero-write filesystem mode. Do not open a
store already owned by the desktop or another process.

A queued tool response is not completed processing. Poll `geod_job_status` until
the job is terminal and `settled: true`; only `succeeded` plus settlement supports
an output-complete result. Protocol input frames are capped at 64 KiB, tool
arguments at 8192 bytes and concurrent calls at eight; core processing limits and
source checks remain in force. Full configuration, tool names, lifecycle details
and verified protocol coverage are in [the MCP guide](../../docs/mcp.md).

本地 OSM 0.6 XML / PBF 快照使用独立原文件来源记录，支持 raw / zlib、dense / non-dense、完整对象引用和 LocationsOnWays；保留 XML / PBF 原件与全部标签 / 编辑元数据。范围、独立比较和离线恢复见[OSM 原文件](../../docs/osm-local-data.md)。


## Scientific RGB files

`scientific-rgb plan|run --request FILE` takes three completed local red/green/blue job IDs, an optional project ID and result name. The `run` CLI waits for settlement. `inspect|package --id UUID` and `pixel --id UUID --x X --y Y` read the completed scientific RGB's own file, even after parent files have been removed. Both exclusive `--data-dir` and existing loopback `--server` backends reuse the same task core.

The matching HTTP endpoints are POST `/rasters/rgb/plan`, POST `/rasters/rgb`, GET `/jobs/{id}/rgb` and GET `/jobs/{id}/rgb/pixel?x=&y=`. Existing job, file, thumbnail, provenance and package routes support the new `raster_rgb` kind and `reflectance_rgb` key. MCP provides three read tools and two explicitly enabled write tools. Supported product identities, original/processed grid matching, calibration, file/disk limits, independent complete-file checks and protected-source boundaries are in [scientific RGB acceptance](../../docs/scientific-rgb.md).
