# GeoD Global local MCP

The `geod-runtime serve-mcp` command exposes the same persisted jobs, raster inspection, pixel lookup and executable recipes used by the desktop application and CLI. It is a local stdio adapter built with the official [`rmcp` Rust SDK](https://github.com/modelcontextprotocol/rust-sdk), pinned to `3.4.0`. It does not start a second processing implementation or a public MCP endpoint.

## Connect to an existing local runtime

Use this mode while the browser runtime owns the data directory:

```powershell
geod-runtime serve-mcp --server http://127.0.0.1:4318
```

The service at port 4318 must already be running. An example MCP client configuration is:

```json
{
  "mcpServers": {
    "geod-global": {
      "command": "geod-runtime",
      "args": ["serve-mcp", "--server", "http://127.0.0.1:4318"]
    }
  }
}
```

Put the binary on your PATH or replace `command` with its absolute path. For a source checkout, build with `cargo build --locked -p geod-runtime` and use `target/debug/geod-runtime.exe` on Windows or `target/debug/geod-runtime` on Linux/macOS. Pass the executable directly to the client: a command wrapper that writes status messages to stdout would corrupt the protocol.

Writes are disabled by default. To let the client download data, save/run recipes or change task state, add `--allow-write` to the process arguments. This setting is checked in both tool discovery and tool dispatch; tool arguments cannot enable it. Clients should still provide their normal user controls for mutating tool calls.

`--server` accepts only an HTTP `127.0.0.1` origin. Credentials, URL paths, query strings, fragments, remote hosts, proxy routing and redirects are rejected. This is an adapter to GeoD's existing local REST service, **not** an MCP Streamable HTTP server.

## Own a separate data directory

For a standalone MCP process:

```powershell
geod-runtime serve-mcp --data-dir ./geod-agent-data
geod-runtime serve-mcp --data-dir ./geod-agent-data --allow-write
```

Choose exactly one of `--server` or `--data-dir`. The data directory has an exclusive process lock. Do not point standalone MCP at a directory already owned by the desktop, browser service, CLI, or another MCP process. Starting an independent empty directory is supported; its job and recipe lists start empty.

The desktop currently owns its runtime internally and does not expose its own HTTP endpoint. Close the desktop before opening its store directly, or use a separate standalone store. `--server` connects to the explicitly running loopback service; it does not silently discover or control a desktop window.

The default controls which **tools** may write. Opening a direct store still creates/locks the directory and performs normal interrupted-job recovery. It is not a forensic, zero-write filesystem viewer.

## Available tools

| Tool | Default | Result |
| --- | --- | --- |
| `geod_health` | Read | Runtime status, storage location, limits and ownership behavior |
| `geod_jobs_list` | Read | Jobs, newest first; optional `offset` and `limit` (default 20, maximum 100) |
| `geod_job_status` | Read | One job and its worker `settled` flag |
| `geod_raster_inspect` | Read | Verified SCL checksum, dimensions, CRS, bounds, resolution and class counts; PNG is omitted |
| `geod_raster_pixel` | Read | Original pixel value/class, zero-based column/row and pixel center at an `x`,`y` coordinate in the raster's source CRS |
| `geod_recipes_list` | Read | Saved executable recipes; optional `offset` and `limit` |
| `geod_recipe_plan` | Read | Actual output dimensions, projected bounds and pixel window for a pinned recipe |
| `geod_download` | `--allow-write` | Queue an allowlisted Sentinel asset download |
| `geod_recipe_run` | `--allow-write` | Queue a real local SCL GeoTIFF crop |
| `geod_recipe_save` | `--allow-write` | Validate and persist a recipe |
| `geod_job_cancel` | `--allow-write` | Request job cancellation; polling establishes cleanup completion |
| `geod_job_retry` | `--allow-write` | Retry an eligible failed/cancelled/interrupted job |

Job tools use `{"id":"lowercase-hyphenated-uuid"}`. Pixel lookup adds numeric `x` and `y`. Recipe tools use `{"recipe":{...}}` with the complete [executable recipe contract](../schemas/raster-recipe-v1.schema.json). Download uses `{"request":{...}}` matching the [Sentinel example](../examples/sentinel-scl-download.json). Unknown arguments are rejected at every input layer.

Jobs and recipes are the runtime's persisted records, not the design prototype's simulation cards. SCL is Sentinel's scene classification layer, not a land-cover product. Current processing is rectangular clipping of supported single-band UInt8 SCL GeoTIFFs; see the [raster workflow](workflows/clip-sentinel-scl.md) for processing limits.

## A complete agent workflow

1. Read `geod_health`, then `geod_jobs_list`. Pick an existing completed SCL job and its SHA-256.
2. Call `geod_raster_inspect` to establish the source CRS and extent. Call `geod_raster_pixel` only with coordinates in that CRS.
3. Construct a `geod-raster-recipe/v1` recipe with that exact job ID and checksum. Use `EPSG:4326` for a longitude/latitude crop rectangle, or `source` for a rectangle in the inspected CRS.
4. Call `geod_recipe_plan` and review the actual pixel-aligned dimensions/bounds and warnings.
5. With writes explicitly enabled, call `geod_recipe_run`. The response contains `jobId`, a current snapshot, the polling call and session ownership behavior. **Acceptance is not completion.**
6. Poll `geod_job_status` until `settled` is true and status is terminal. Only `status: "succeeded"` plus `settled: true` establishes a completed output. Then inspect the derived job or use its persisted artifact path in the local application.

Recipe files pin a local source record. Importing a recipe does not download a missing source, guess its location, or accept a changed checksum. The repository's example recipe references the verified development dataset; replace both source job ID and checksum when using a different store.

### Disconnect and cancellation

In `--server` mode the service owns tasks. Closing an MCP session leaves accepted jobs running in that service. Reconnect and poll the returned ID, or explicitly request cancellation. An MCP request cancellation only abandons that response; it does not imply `geod_job_cancel`.

In `--data-dir` mode the MCP process owns tasks. Keep the session open until they settle. On stdin EOF or Ctrl-C the adapter stops accepting calls, drains already-started bounded calls, cancels active jobs, and waits for worker cleanup before exit. Interrupting a response cannot drop a half-persisted write future. A hard process kill cannot guarantee cleanup; normal runtime recovery marks unfinished records interrupted on the next open.

Cancelled status can appear before a worker has finished deleting its partial output, so always wait for `settled: true` before retrying. Operational errors use a tool result with `isError: true`; malformed/unknown/disabled tools use JSON-RPC errors. Neither means processing succeeded.

## Boundaries and protocol

- The SDK handles MCP initialization, protocol negotiation, JSON-RPC methods and tool responses. Messages are UTF-8 JSON separated by newlines; stdout contains only protocol messages and diagnostics go to stderr, following the [stdio specification](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports).
- Each incoming frame is limited to 64 KiB before JSON parsing. Tool arguments are limited to 8192 bytes; at most 8 calls run concurrently. HTTP responses are capped at 4 MiB, returned tool data at 512 KiB. Use a smaller list limit if a result is too large.
- Core limits remain in force: 64 active/queued jobs, 512 MiB per downloadable asset, one raster worker shared across inspection/planning/cropping/pixel reads, and supported raster limits of 128 MiB, 64 Mi pixels and 16,384 pixels per edge. The adapter cannot override them.
- Tools accept no shell commands, arbitrary file paths, output destinations or remote server choices. Only the existing unsigned HTTPS Sentinel bucket allowlist can be downloaded. File signatures, byte counts, source/output hashes, recipe pins and supported raster georeferencing are checked by the same Rust runtime.
- Read tools perform no third-party network requests. Local file paths, source metadata and exact crop coordinates may be returned to the chosen MCP client. GeoD adds no telemetry or external upload; any onward handling by that client follows the client's own configuration.
- Tool safety annotations describe these operations; they are hints, not authorization. Source titles and returned metadata must be treated as data, never as instructions. The adapter exposes no sampling, roots, arbitrary resources or remote access capabilities.

## Verification

Run the bounded unit/protocol suite:

```powershell
cargo test --locked -p geod-runtime mcp::tests --lib
```

Against the already running local service with the example's pinned source available:

```powershell
python scripts/verify-mcp.py --executable target/debug/geod-runtime.exe --server http://127.0.0.1:4318 --recipe examples/sentinel-scl-clip.recipe.json --report .verification/mcp-readonly.json
python scripts/verify-mcp.py --executable target/debug/geod-runtime.exe --server http://127.0.0.1:4318 --recipe examples/sentinel-scl-clip.recipe.json --report .verification/mcp-processing.json --allow-write
```

The second command deliberately creates one real crop. The protocol client verifies read-only enforcement, structured/text result agreement, source inspection, crop planning, pixel reads, checksum-mismatch rejection, explicit write discovery, a settled real output, persistence across MCP reconnect, protocol-only stdout and clean EOF exits. Independent pixel/GeoTIFF verification remains in `scripts/verify-raster-crop.py`.

Verified on Windows on 2026-09-22 using that command: MCP negotiated `2025-11-25`, produced job `26119774-870e-4b97-9f27-7fa8e6b907cb` at **1020 × 895** pixels, and reported `succeeded` with `settled: true`. The output SHA-256 was `af78deb054871d1f6b5c02de84f40fddbeac060ecd5d79781c305a7aa4063ecd`. Independent Rasterio/GDAL validation matched all **912,900** output samples, the source window, CRS/transform/nodata and provenance sidecar; the source file was unchanged. The local reports are `.verification/mcp-processing.json` and `.verification/mcp-crop-independent.json` (generated verification output, not required runtime dependencies).

Unit coverage also checks frame caps, invalid configuration and paths, operational errors, denied writes, cancelled response draining, direct-mode cancellation cleanup, omitted image payloads and SDK protocol errors. This adapter is the local tool surface; hosted MCP, authentication, cloud processing and general GIS operations remain separate work.
