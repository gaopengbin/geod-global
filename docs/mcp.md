# GeoD Global local MCP

The `geod-runtime serve-mcp` command exposes the same persisted jobs, projects, custom STAC selections, WCS coverage selections, raster inspection, pixel lookup, scientific RGB and executable recipes used by the desktop application and CLI. It is a local stdio adapter built with the official [`rmcp` Rust SDK](https://github.com/modelcontextprotocol/rust-sdk), pinned to `3.4.0`. It does not start a second processing implementation or a public MCP endpoint.

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

Writes are disabled by default. To let the client connect/search a custom STAC source, connect a WCS source, persist metadata/plans/projects, download data, save/run recipes or change task state, add `--allow-write` to the process arguments. STAC search persists snapshots and consumes cursors, so it is a mutating metadata operation even though it downloads no raster. This setting is checked in both tool discovery and tool dispatch; tool arguments cannot enable it. Clients should still provide their normal user controls for mutating tool calls.

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

Saved vector services and local vector files add six read-only tools. They share the native verification and bounded node reader, and do not connect, query, extract or modify data. Tool names, paging, Agent redaction and real public/offline evidence are in [Vector MCP and Agent integration](vector-agent.md).

## Available tools

Custom sources add six reads (`geod_stac_connections`, `geod_stac_catalog`, `geod_stac_snapshot`, `geod_stac_assets`, `geod_stac_inspect`, `geod_stac_pixel`) and five opted-in mutations (`geod_stac_connect`, `geod_stac_search`, `geod_stac_project_save`, `geod_stac_download`, `geod_stac_forget`). Their exact request examples, source semantics and fresh public/offline verification are in [STAC MCP and Agent integration](stac-mcp.md). Item/asset metadata is preserved natively, never interpreted as instructions; search summaries retain every returned identity and continuation while details are read separately. These external MCP mutations are not added to the desktop Agent's model allowlist.

| Tool | Default | Result |
| --- | --- | --- |
| `geod_health` | Read | Runtime status, storage location, limits and ownership behavior |
| `geod_jobs_list` | Read | Jobs, newest first; optional `projectId` filters exact native sources and processing lineage before pagination. Scoped results include fresh settlement and `checkedAt`; without it, the existing global list remains unchanged. Optional `offset` and `limit` (default 20, maximum 100) |
| `geod_job_status` | Read | One job and its worker `settled` flag |
| `geod_raster_inspect` | Read | Verified supported RGB / SCL / reflectance / NAIP / GLO-30 Public checksum, geometry and product metadata; PNG is omitted |
| `geod_raster_pixel` | Read | Original samples, zero-based column/row and pixel center in the source CRS; NAIP includes raw RGB and `nearInfrared` |
| `geod_recipes_list` | Read | Saved executable recipes; optional `offset` and `limit` |
| `geod_recipe_plan` | Read | Actual output dimensions, projected bounds and pixel window for a pinned recipe |
| `geod_download` | `--allow-write` | Queue a reviewed source asset through the shared native provider adapter |
| `geod_recipe_run` | `--allow-write` | Queue a real local SCL GeoTIFF crop |
| `geod_recipe_save` | `--allow-write` | Validate and persist a recipe |
| `geod_job_cancel` | `--allow-write` | Request job cancellation; polling establishes cleanup completion |
| `geod_job_retry` | `--allow-write` | Retry an eligible failed/cancelled/interrupted job |
| `geod_projects_list` | Read | Paginated project summaries with saved scene / STAC / WCS counts |
| `geod_project_get` | Read | One saved project's AOI and selections |
| `geod_wcs_connections` | Read | Paginated saved connection summaries; no external requests |
| `geod_wcs_coverages` | Read | Paginated archived catalog and service declarations |
| `geod_wcs_description` | Read | Immutable coverage description by saved SHA-256 ID |
| `geod_wcs_plan` | Read | Immutable native-grid request by saved SHA-256 ID |
| `geod_wcs_inspect` | Read | Verified local subset grid, sample types and file checksum; PNG omitted |
| `geod_wcs_pixel` | Read | Original TIFF samples by zero-based `column` / `row` |
| `geod_wcs_connect` | `--allow-write` | Contact and save a user-selected public HTTPS WCS 2.0.1 KVP service |
| `geod_wcs_describe` | `--allow-write` | Contact the saved service and persist an advertised coverage definition |
| `geod_wcs_prepare` | `--allow-write` | Persist a local native-grid plan; no external requests |
| `geod_wcs_project_save` | `--allow-write` | Create or append saved plan references to a project |
| `geod_wcs_download` | `--allow-write` | Queue project subsets or reuse checksum-verified completed files |
| `geod_wcs_forget` | `--allow-write` | Remove a connection from discovery; retain saved plans, projects and files |
| `geod_rgb_plan` | Read | Verify three completed red/green/blue jobs and report the original grid, calibration and disk budget |
| `geod_rgb_inspect` | Read | Verify a saved scientific RGB file and its source specification; display PNG omitted |
| `geod_rgb_pixel` | Read | Read original per-channel DN, NoData and reflectance at source-CRS x/y |
| `geod_rgb_run` | `--allow-write` | Queue an Int16/UInt16 scientific RGB GeoTIFF from pinned local bands |
| `geod_rgb_package` | `--allow-write` | Prepare a verified local ZIP containing the file, display preview, provenance and checksums |

There are 30 read tools and 18 additional mutation tools. Job/project/connection tools use `{"id":"lowercase-hyphenated-uuid"}`; saved WCS description/plan IDs instead use 64 lowercase SHA-256 characters. Sensor-specific pixel lookup adds numeric source-CRS `x` and `y`; WCS pixel lookup adds integer TIFF `column` and `row`. Recipe tools use `{"recipe":{...}}` with the complete [executable recipe contract](../schemas/raster-recipe-v1.schema.json). Download uses `{"request":{...}}` matching the [Sentinel example](../examples/sentinel-scl-download.json). WCS mutations also use a `request` envelope, with the native contracts in [WCS coverage subsets](wcs-coverages.md). Unknown arguments are rejected at every input layer.

Jobs and recipes are the runtime's persisted records, not the design prototype's simulation cards. SCL is Sentinel's scene classification layer, not a land-cover product. Current processing is rectangular clipping of supported single-band UInt8 SCL GeoTIFFs; see the [raster workflow](workflows/clip-sentinel-scl.md) for processing limits.

## A complete agent workflow

1. Read `geod_health`, then `geod_jobs_list`. Pick an existing completed SCL job and its SHA-256.
2. Call `geod_raster_inspect` to establish the source CRS and extent. Call `geod_raster_pixel` only with coordinates in that CRS.
3. Construct a `geod-raster-recipe/v1` recipe with that exact job ID and checksum. Use `EPSG:4326` for a longitude/latitude crop rectangle, or `source` for a rectangle in the inspected CRS.
4. Call `geod_recipe_plan` and review the actual pixel-aligned dimensions/bounds and warnings.
5. With writes explicitly enabled, call `geod_recipe_run`. The response contains `jobId`, a current snapshot, the polling call and session ownership behavior. **Acceptance is not completion.**
6. Poll `geod_job_status` until `settled` is true and status is terminal. Only `status: "succeeded"` plus `settled: true` establishes a completed output. Then inspect the derived job or use its persisted artifact path in the local application.

Recipe files pin a local source record. Importing a recipe does not download a missing source, guess its location, or accept a changed checksum. The repository's example recipe references the verified development dataset; replace both source job ID and checksum when using a different store.

### WCS agent workflow

1. With writes enabled, call `geod_wcs_connect` with `{"request":{"name":"User-selected source","url":"https://…/wcs"}}`. Page through `geod_wcs_coverages` using the returned connection ID; reading the saved catalog causes no external requests.
2. Call `geod_wcs_describe` with `{"request":{"connectionId":"UUID","coverageId":"advertised coverage ID"}}`. Review native axes, grid, fields, units, nil values and warnings. Service access constraints of `NONE` do not establish an asset license.
3. Call `geod_wcs_prepare` with `{"request":{"descriptionId":"SHA256","bounds":[west,south,east,north]}}`. This saves a local plan without downloading pixels. Review the actual aligned native bounds and dimensions; no automatic scaling or reprojection occurs.
4. Read `geod_projects_list` and `geod_project_get` if appending to an existing project. Call `geod_wcs_project_save` with `{"request":{"projectId":"UUID","bounds":[west,south,east,north],"selections":[{"planId":"SHA256"}]}}`; omit `projectId` and add `name` for a new project. The existing project name and AOI are retained.
5. Call `geod_wcs_download` with `{"request":{"projectId":"UUID","selections":[{"planId":"SHA256"}]}}`; omit `selections` to request all WCS plans in that project. The result contains a polling call for each queued or reused job. Poll every job until `settled: true`; only `status: "succeeded"` establishes completed output.
6. Call `geod_wcs_inspect` and `geod_wcs_pixel` with the completed job ID. Keep source range declarations separate from actual TIFF tags and samples. Large integers outside JavaScript's safe range and non-finite numbers retain string representations, rather than silently changing values.

The file is a **server-generated coverage subset**, not an original survey archive or automatically calibrated scientific product. Transfer URL, sample dimensions, grid and source pins are derived from immutable native records; MCP accepts no replacement `href`, account secrets or output path. The underlying [WCS limits](wcs-coverages.md) remain in force. Both existing-file reuse and a fresh public acquisition through actual MCP have separate acceptance records below.

### Disconnect and cancellation

In `--server` mode the service owns tasks. Closing an MCP session leaves accepted jobs running in that service. Reconnect and poll the returned ID, or explicitly request cancellation. An MCP request cancellation only abandons that response; it does not imply `geod_job_cancel`.

In `--data-dir` mode the MCP process owns tasks. Keep the session open until they settle. On stdin EOF or Ctrl-C the adapter stops accepting calls, drains already-started bounded calls, cancels active jobs, and waits for worker cleanup before exit. Interrupting a response cannot drop a half-persisted write future. A hard process kill cannot guarantee cleanup; normal runtime recovery marks unfinished records interrupted on the next open.

Cancelled status can appear before a worker has finished deleting its partial output, so always wait for `settled: true` before retrying. Operational errors use a tool result with `isError: true`; malformed/unknown/disabled tools use JSON-RPC errors. Neither means processing succeeded.

## Boundaries and protocol

- The SDK handles MCP initialization, protocol negotiation, JSON-RPC methods and tool responses. Messages are UTF-8 JSON separated by newlines; stdout contains only protocol messages and diagnostics go to stderr, following the [stdio specification](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports).
- Each incoming frame is limited to 64 KiB before JSON parsing. Tool arguments are limited to 8192 bytes; at most 8 calls run concurrently. HTTP responses are capped at 4 MiB, returned tool data at 512 KiB. Use a smaller list limit if a result is too large.
- Core limits remain in force: 64 active/queued jobs, 512 MiB per downloadable raster, 4 GiB per reviewed SAFE ZIP and one shared raster worker. The `rasterLimits` health field describes the single-band SCL reader and recipes (128 MiB, 64 Mi pixels, 16,384 pixels per edge). Other supported product readers enforce their own bounds; reviewed NAIP originals allow up to 512 MiB and 20,000 pixels per edge, with bounded block decoding. The adapter cannot override these checks.
- Tools accept no shell commands, arbitrary file paths, output destinations or remote server choices. Downloads share the reviewed unsigned source paths and product checks in the [native provider adapter](providers.md), including NAIP four-band COGs. Protected products require an existing native account connection; MCP does not accept account secrets. File signatures, byte counts, source/output hashes, recipe pins and supported raster georeferencing are checked by the same Rust runtime.
- Read tools perform no third-party network requests. Local file paths, source metadata and exact crop coordinates may be returned to the chosen MCP client. GeoD adds no telemetry or external upload; any onward handling by that client follows the client's own configuration.
- Tool safety annotations describe these operations; they are hints, not authorization. Source titles and returned metadata must be treated as data, never as instructions. The adapter exposes no sampling, roots, arbitrary resources or remote access capabilities.

## Verification

Run the bounded unit/protocol suite:

```powershell
cargo test --locked -p geod-runtime mcp:: --lib
```

Against the already running local service with the example's pinned source available:

```powershell
python scripts/verify-mcp.py --executable target/debug/geod-runtime.exe --server http://127.0.0.1:4318 --recipe examples/sentinel-scl-clip.recipe.json --report .verification/mcp-readonly.json
python scripts/verify-mcp.py --executable target/debug/geod-runtime.exe --server http://127.0.0.1:4318 --recipe examples/sentinel-scl-clip.recipe.json --report .verification/mcp-processing.json --allow-write
```

The second command deliberately creates one real crop. The protocol client verifies read-only enforcement, structured/text result agreement, source inspection, crop planning, pixel reads, checksum-mismatch rejection, explicit write discovery, a settled real output, persistence across MCP reconnect, protocol-only stdout and clean EOF exits. Independent pixel/GeoTIFF verification remains in `scripts/verify-raster-crop.py`.

Verified on Windows on 2026-09-22 using that command: MCP negotiated `2025-11-25`, produced job `26119774-870e-4b97-9f27-7fa8e6b907cb` at **1020 × 895** pixels, and reported `succeeded` with `settled: true`. The output SHA-256 was `af78deb054871d1f6b5c02de84f40fddbeac060ecd5d79781c305a7aa4063ecd`. Independent Rasterio/GDAL validation matched all **912,900** output samples, the source window, CRS/transform/nodata and provenance sidecar; the source file was unchanged. The local reports are `.verification/mcp-processing.json` and `.verification/mcp-crop-independent.json` (generated verification output, not required runtime dependencies).

Unit coverage also checks frame caps, invalid configuration and paths, operational errors, denied writes, cancelled response draining, direct-mode cancellation cleanup, omitted image payloads and SDK protocol errors. This adapter is the local tool surface; hosted MCP, authentication, cloud processing and general GIS operations remain separate work.

### Real WCS MCP acceptance

A later fresh-store run completed public EMODnet connection, catalog paging, coverage description, native-grid preparation, project save and a new GetCoverage transfer through write-enabled loopback stdio MCP. The eight catalog entries matched the original capabilities XML. The 48 × 48, 9,600-byte Float32 subset matched a second independent public response byte for byte; Rasterio verified all 2,304 unscaled samples and the grid. Five online MCP pixel reads passed. After stopping the runtime, five standalone/loopback offline sessions and 25 further independent pixel checks passed, with unchanged registries and zero attempts through a rejecting proxy. The Windows CLI now boxes the nested MCP startup future after this workflow exposed a standalone initialization stack overflow. A fresh download and recovery passed with the fix; active unrelated download processes were preserved. See the [fresh MCP acquisition receipt](../prototype/qa/wcs-mcp-public-verification.json). To repeat, use a new directory under `.verification` and a public service selected explicitly:

```powershell
python -X utf8 scripts/verify-wcs-mcp-public.py --root .verification/wcs-mcp-public-new --executable target/debug/geod-runtime.exe --endpoint https://ows.emodnet-bathymetry.eu/wcs --coverage emodnet__mean --bounds "2,53,2.05,53.05"
python -X utf8 scripts/summarize-wcs-mcp-public.py .verification/wcs-mcp-public-new
```

The receipt keeps the service's questionable `W.m-2.Sr-1` field unit separate from the TIFF's missing unit tag; it does not invent metres or establish the linked dataset license. It does not claim every WCS service, every Agent client or an installed desktop WebView has passed.

On 2026-10-03, five actual stdio sessions passed against the completed EMODnet test subset: direct store reads/writes, loopback service reads/writes and direct reconnect. The saved eight-entry catalog was fully read through one-entry pages; immutable descriptions/plans and the project survived unchanged. Local plan preparation, idempotent project append and checksum-verified download reuse succeeded. All 25 original-pixel reads matched independent Rasterio decoding. PNG payloads were excluded from tool text, read-only discovery/dispatch denied all six WCS mutations, and SDK sessions closed cleanly on stdin EOF. A rejecting local proxy recorded **zero** external attempts; no new public coverage or metadata request was made. This verifies reuse through MCP, not a new remote acquisition through those sessions or every Agent client's integration.

The [MCP receipt](../prototype/qa/wcs-mcp-verification.json) records the binary hash and session results. Six new WCS MCP regression tests passed together with existing lifecycle/protocol tests; the complete native workspace passed 337 tests with 4 ignored. To repeat against a deliberate verification store containing an actual completed WCS file, install Rasterio for the independent checks and use:

```sh
python scripts/verify-wcs-mcp.py --executable target/debug/geod-runtime.exe --data-dir .verification/wcs-runtime --project PROJECT_UUID --job JOB_UUID --plan PLAN_SHA256 --report .verification/wcs-mcp/report.json
```

The verifier only accepts a store under `.verification`. It temporarily routes that store's source requests to a rejecting local proxy, restores its original proxy configuration, owns the short-lived verification service and never changes the desktop's data directory. It reuses the existing file; it does not select an upstream endpoint or account.


### Scientific RGB workflow and real acceptance

Call `geod_rgb_plan` with `{"request":{"jobIds":["RED_UUID","GREEN_UUID","BLUE_UUID"],"projectId":"OPTIONAL_UUID","name":"OPTIONAL_NAME"}}`. Review the actual matched source grid, calibration, source checksums and required disk space. With writes enabled, submit that request to `geod_rgb_run`, then poll the returned `geod_job_status` call until terminal and `settled: true`. Only `succeeded` establishes a usable result. Inspect it with `geod_rgb_inspect`, read raw values with `geod_rgb_pixel` using source-CRS `x` / `y`, and prepare its local ZIP with `geod_rgb_package`.

The tools reuse the same bounded native core as the desktop/CLI. Three reviewed product bands must have exactly matching grids and scene or processed-project provenance; no implicit resampling occurs. The GeoTIFF keeps 16-bit DN, per-band calibration, NoData and CRS. Completed RGB files work without their parent files; recreating a file still requires checked parents. See [scientific RGB limits and evidence](scientific-rgb.md).

On 2026-10-03, real stdio sessions exercised direct-directory and loopback-service modes against the actual MODIS/Landsat acceptance store. Read-only discovery and dispatch rejected both RGB mutations. Valid preflight, saved-file inspection without parent files, independently checked pixels, two actual new MODIS RGB files and verified delivery ZIPs passed. Wrong band order and repeated IDs were rejected. Direct EOF/reopen retained the file, and a loopback job survived disconnect before settlement and completed after reconnection. All stdout remained protocol JSON and normal EOF exits were clean. The [receipt](../prototype/qa/scientific-rgb-verification.json) records binary/file hashes and exact results. This does not establish every Agent client's integration or positive NASA/Copernicus product authorization.

MOD/MYD09A1 v061 quality COGs are also supported by `geod_raster_inspect` and `geod_raster_pixel`. Inspection returns exact UInt32 QC or UInt16 State geometry, original full-resolution class counts and the official bit definition. Pixel sampling retains the unsigned raw value, hexadecimal / binary representation and every decoded field, without reflectance scaling or cloud masking. Real read-only direct and loopback sessions checked both original files against independently compared samples; a disconnected adapter rejected access. Details and concrete limits are in [MODIS quality acceptance](modis-quality.md) and its [receipt](../prototype/qa/modis-quality-verification.json). These tools do not imply support for other MODIS products or quality-layer processing.

Scientific RGB also accepts a strict product-specific `qualityMask`: MODIS uses `qcJobId` / `stateJobId` and `clear` / `clear_best`; Landsat uses `qaPixelJobId` / `qaRadsatJobId` and `cloud_free` / `cloud_free_conservative`. Both keep optional `excludeSnow` and default to retaining all original DN when the object is absent. The discovery schema exposes these alternatives through `oneOf`; mixed fields and policies are rejected. Real Landsat 9 direct and loopback sessions created two same-scene clipped RGB files, independently checked all 40,309,920 DN and six raw queries, enforced read-only discovery, and recovered a job across adapter reconnection. Details and limits are in [Landsat screening](landsat-rgb-quality-mask.md) and the [bound receipt](../prototype/qa/landsat-rgb-mask-verification.json), which retains its initial same-scene scope.

Matched Landsat multi-scene project layers now use the same request with a version-two quality specification that pins all five original files per scene. Two new direct / loopback MCP outputs retained complete qualified RGB triplets together, independently matched all 5,544 DN and six raw queries, enforced read-only discovery and rejected invalid rules / duplicate QA IDs. Loopback processing survived adapter disconnect and was readable and deliverable after reconnect. See [coherent Landsat selection](landsat-coupled-rgb.md) and the [new bound receipt](../prototype/qa/landsat-coupled-verification.json). Positive protected-product authorization remains pending.
