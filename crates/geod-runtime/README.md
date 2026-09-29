# GeoD Global local runtime

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

## Transfer behavior

- Only unsigned HTTPS assets on
  `sentinel-cogs.s3.us-west-2.amazonaws.com/sentinel-s2-l2a-cogs/` are accepted.
  `itemId` must occur as a complete URL path segment and the URL extension must
  agree with the TIFF or JPEG media type. Redirects are never followed.
- At most two transfers run concurrently; up to 64 active or queued jobs are
  accepted. Each transfer is limited to 512 MiB and 30 minutes, with a 45-second
  read inactivity timeout.
- Data streams into a `.part` file while SHA-256 and byte count are calculated.
  Success requires a recognized TIFF/JPEG signature and an exact Content-Length
  match when the server supplies it. The file is synced and atomically renamed
  before success is recorded.
- These are **file signature and transfer integrity checks**, not scientific
  raster validation, source authenticity checks, or full TIFF/JPEG decoding.
- Cancellation stops the transfer. Explicit retry uses the same job ID and
  starts at byte zero. No byte-range continuation is implemented.
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
| GET | `/jobs` | Array of persisted jobs, newest first |
| GET | `/jobs/{id}` | One persisted job plus `settled`, or 404 |
| POST | `/jobs` | `202` and a queued job |
| POST | `/jobs/{id}/cancel` | Updated job |
| POST | `/jobs/{id}/retry` | Queued retry, or error if still settling |
| GET | `/jobs/{id}/raster` | Verified local SCL metadata, class counts and PNG preview |
| GET | `/jobs/{id}/pixel?x=...&y=...` | Full-resolution SCL pixel at finite source-CRS coordinates |
| GET | `/jobs/{id}/file` | Revalidated derived GeoTIFF as an attachment |
| GET | `/jobs/{id}/metadata` | Revalidated project-mosaic provenance JSON as an attachment |
| GET | `/projects` | Persisted multi-scene projects |
| POST | `/projects` | Save selected scenes, area and optional polygon |
| POST | `/projects/{id}/downloads` | Queue or reuse every `scl` or `visual` source download in the project |
| POST | `/projects/{id}/mosaics` | Queue a pinned, pixel-aligned mosaic and area clip for `scl` or `visual` |
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
their approved SCL/true-color asset URLs, WGS84 search bounds and an optional
WGS84 polygon. Each asset type is downloaded into ordinary jobs, with completed
or active matching source jobs reused. A `raster_mosaic` job pins every source
job ID and SHA-256 and produces its own GeoTIFF plus `.metadata.json` manifest.
Its output is limited to 8 million pixels and 128 MiB. Source files remain
unchanged. The newest non-NoData pixels win, and polygon masking uses output
pixel centres. The output preserves its UTM grid, source band type and pixel
spacing; different UTM zones or unaligned grids fail explicitly. SCL is
single-band UInt8; true-color TCI is three-band UInt8. No reprojection or
resampling is performed. The raw result can be downloaded by the browser or
revealed in the desktop file explorer. The existing ZIP bundle contract still
covers single-source `raster_clip` outputs only.

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

## MCP stdio adapter

`serve-mcp` uses the official `rmcp` SDK, pinned to 3.4.0, with the same Rust
manager and strict job/recipe inputs. Choose exactly one ownership mode:

```sh
geod-runtime serve-mcp --server http://127.0.0.1:4318
geod-runtime serve-mcp --data-dir ./agent-runtime-data
```

Seven read/inspection/preflight tools are exposed by default. The explicit startup
flag `--allow-write` enables five additional download, recipe and task mutation
tools; arguments cannot enable them. This is local newline-delimited JSON-RPC on
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
