# GeoD Global local runtime

This crate is independent of the domestic application. The Tauri application and
the loopback development adapter use the same `JobManager` implementation.

```sh
cargo test -p geod-runtime
cargo run -p geod-runtime -- serve --data-dir .verification/runtime-data --port 4318
```

The storage directory is mandatory for the CLI. Desktop code supplies its own
application data directory. A process lock prevents two runtimes from writing the
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
| POST | `/jobs` | `202` and a queued job |
| POST | `/jobs/{id}/cancel` | Updated job |
| POST | `/jobs/{id}/retry` | Queued retry, or error if still settling |
| GET | `/jobs/{id}/raster` | Verified local SCL metadata, class counts and PNG preview |

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
fresh integrity check. A semaphore admits one blocking inspection at a time;
other requests receive a busy error. The decoder has explicit allocation limits,
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
