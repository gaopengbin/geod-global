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
same storage directory. All output filenames are generated UUIDs under `assets/`;
the request cannot supply a destination path.

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
| GET | `/jobs` | Array of persisted jobs, newest first |
| GET | `/jobs/{id}` | One persisted job, or 404 |
| POST | `/jobs` | `202` and a queued job |
| POST | `/jobs/{id}/cancel` | Updated job |
| POST | `/jobs/{id}/retry` | Queued retry, or error if still settling |
| GET | `/jobs/{id}/raster` | Verified local SCL metadata, class counts and PNG preview |
| GET | `/recipes` | Saved recipes, newest first |
| POST | `/recipes/plan` | `{ recipe, plan }`; validates and computes an actual pixel window without writing files |
| POST | `/recipes` | `201` and `{ id, recipe, createdAt, updatedAt }`; validates the source and plan before saving |
| POST | `/recipes/run` | `202` and a queued `raster_clip` job; execution freshly verifies the pinned source |

Create requests use `{ "itemId", "assetKey", "href", "mediaType", "title" }`;
`title` is optional. JSON responses use camelCase and status values
`queued`, `running`, `succeeded`, `failed`, `cancelled`, `interrupted`.

The deterministic tests use a local TCP fixture **only under `cfg(test)`**. The
production API has no flag, environment variable or localhost URL override.

## Native SCL raster inspection

`JobManager::inspect_raster(id)` and desktop `inspect_raster { id }` inspect a
completed SCL GeoTIFF, with no external processing executable. The pinned pure
Rust `tiff` 0.11.3 and `png` 0.18.1 libraries decode the image and encode its preview.
The original asset remains unchanged.

Each request resolves the job's UUID filename inside its canonical managed
`assets` directory, checks its recorded byte count and recalculates SHA-256.
The exact verified bytes are then decoded, so cached metadata never bypasses a
fresh integrity check. A shared semaphore admits one blocking raster inspection,
plan or clip at a time; interactive inspection/planning requests receive a busy
error while another operation owns it, and clip jobs wait in the queue. The decoder has explicit allocation limits,
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
