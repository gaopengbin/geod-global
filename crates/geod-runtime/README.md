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

Create requests use `{ "itemId", "assetKey", "href", "mediaType", "title" }`;
`title` is optional. JSON responses use camelCase and status values
`queued`, `running`, `succeeded`, `failed`, `cancelled`, `interrupted`.

The deterministic tests use a local TCP fixture **only under `cfg(test)`**. The
production API has no flag, environment variable or localhost URL override.
