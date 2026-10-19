# Actual model dialogue channel acceptance — 2026-10-15

Eight representative channel routes used the saved `deepseek-flash` model, the real bundled Agent and the native download/review engine. This was a native integration test host, not mocked model replies or manually downloaded substitute files. The test driver confirmed only the current small-file native reviews using their actual `planId` and `planHash`. All transfers were fresh. The user's normal workspace and model registry were preserved.

The isolated workspaces reused the saved model credential through the existing vault. Provider accounts and user-created source connections were **not** copied. Results for authenticated or connected-service channels describe this test environment; they do not establish the user's normal desktop account status.

| Channel and requested original | Search / actual download | Independent file check | Workspace result |
| --- | --- | --- | --- |
| Earth Search Sentinel-2 L2A SCL, San Francisco, 2026-09 | Passed, 2,633,064 bytes | Full SHA-256 and all 30,140,100 samples decoded; UInt8, 5490 × 5490 | Verified native open request; renderer not inspected |
| Copernicus DEM GLO-90, one Nepal tile | Passed on retest, 5,315,373 bytes | Full SHA-256 and all 1,440,000 samples decoded; Float32, 1200 × 1200 | Verified native open request; renderer not inspected |
| WorldPop Niue population count, 2020 | Passed on retest, 156,803 bytes | Full SHA-256 and all 52,430 samples decoded; Float32, 214 × 245 | Generic STAC original inspection passes; Agent workspace-open fails |
| CHIRPS v3 final monthly precipitation, 2025-01 | Passed, 21,628,766 bytes | Full SHA-256 and all 17,280,000 samples decoded; Float32, 7200 × 2400 | Generic STAC inspection passes; Agent workspace-open fails |
| JAXA AMSR2 monthly soil moisture, 2025-12 | Passed, 140,042 bytes | Full SHA-256 and all 810,000 samples decoded; Int16, 900 × 900 | Generic STAC inspection passes; Agent workspace-open fails |
| geoBoundaries Niue ADM0 | No saved vector-service connection in the test workspace; no download | Not applicable | Agent reads saved connections but cannot create this connection |
| SoilGrids WCS pH 0–5cm mean subset | No saved WCS connection; no download | Not applicable | Agent cannot establish the requested WCS connection. The built-in SoilGrids archive is a different product and was not substituted |
| Copernicus Data Space SAFE | Source account not configured in the test store; catalog returned invalid JSON on this route; no download | Not applicable | Authenticated original-product download remains unverified |

The SCL and DEM goals passed native delivery checks. Generic-product goals remained incomplete or required attention because opening the result was also requested. A completed model reply is not treated as a completed workflow. None of the workspace-open receipts above proves that map pixels were actually rendered in the desktop window.

Six files were downloaded across initial and retest runs, representing **five distinct originals**: WorldPop was downloaded afresh twice and its complete hashes matched. Independent decoding used Rasterio/GDAL across every block and band, in addition to the runtime's byte-count/signature/hash checks. Unmasked samples are only those not excluded by embedded TIFF NoData; this is not scientific QA. CHIRPS has no embedded NoData tag, so its published -9999 missing-value rule must still be applied for analysis. JAXA negative error codes remain distinct from scalar NoData and do not represent moisture values. No files were cropped, resampled or scientifically converted in this acceptance run.

## Actual request usage

These are the recorded provider receipts for the listed individual attempts. They include repeated input and cache reads. Cache is a subset of input, and reasoning is a subset of output; neither is added to total again. They are not provider invoices or an estimated fee.

| Route / attempt | Elapsed | Model requests | Total tokens | Cached input tokens |
| --- | ---: | ---: | ---: | ---: |
| Earth Search, initial | 57.17 s | 11 | 229,606 | 189,952 |
| GLO-90, retest | 43.00 s | 10 | 184,077 | 159,104 |
| WorldPop, final retest | 51.81 s | 16 | 301,756 | 249,600 |
| CHIRPS, initial | 83.19 s | 17 | 412,400 | 345,728 |
| JAXA, initial | 94.92 s | 19 | 493,938 | 401,152 |
| geoBoundaries, initial | 42.09 s | 7 | 120,916 | 103,424 |
| WCS, final retest | 16.31 s | 3 | 37,653 | 28,544 |
| SAFE / source-account prerequisite, final retest | 23.56 s | 4 | 65,598 | 58,624 |

This does not establish a controlled before/after reduction for download tasks. Failures, unnecessary tool calls, native confirmation boundaries and result-opening gaps still add model round trips. The generic STAC products incorrectly try fixed-provider project/inspection/open tools before recovering to their actual STAC operations. Request-scope tool selection and a small greeting prompt alone do not resolve those workflow costs.

## Fixes from this run

1. The growing built-in dataset catalog exceeded the native 32 KiB result limit, so `geod_dataset_catalog({})` failed before the model could read product options. It now returns bounded summaries with query/offset/limit and exposes the complete selected product through `datasetId`. The new native test reads every catalog page and every individual product through the actual Agent boundary, verifies no missing/duplicate IDs and checks the existing response limit without loosening it. A live diagnostic changed from a size rejection to ten summaries in approximately 1.8 KiB. A subsequent real-model WorldPop dialogue successfully used the repaired native implementation and downloaded its original.
2. Successful capability/status reads no longer count as acquisition progress. This prevents source-account checks, job-status reads or tool discovery alone from sustaining automatic goal continuations after an unsuccessful acquisition. An explicit user continuation remains available.
3. Known schema and bounded-result errors now return narrowly matched corrective guidance. Invalid coverage arguments can be corrected once rather than being mistaken for an outage requiring a new human request. Country-dependent products explicitly require country separately from bounds. Arbitrary upstream errors are still not echoed.
4. Common named product requests activate the dataset/STAC tool family directly, preserving native execution permissions.

The changes passed 155 Agent/runtime tests and six native dataset tests, the dependency-isolation check and a development desktop build. The source-account and service-connection prerequisites were left intact; no credentials were created and no restriction was bypassed. Temporary test hosts were stopped. The normal user desktop session was not restarted and no release was published.

## Remaining gaps

- `geod_workspace_open(kind=raster)` routes only through the fixed-provider inspector and map path. Generic STAC and WCS results need their own verified result type and the corresponding existing map inspector. Downloaded originals must not be described as fully displayed until that path is connected and visually checked.
- Vector/WCS tools can operate saved connections, but the Agent lacks safe reviewed connection creation. Saved-connection workflows were not exercised in this empty-source test workspace.
- Native error recovery and generic-product tool routing need further work to reduce model rounds. This run deliberately records high usage rather than presenting the greeting-only improvement as whole-task optimization.
- This is representative testing of eight routes, not every current product, sensor, band, file format, year, area or mirror. No Landsat or radar transfer, Earth Engine quota use, commercial-image procurement or authenticated SAFE production transfer is claimed here.

Machine-readable [attempts and workflow evidence](../prototype/qa/agent-dialogue-channels-20261015.json) and [independent full-file checks](../prototype/qa/agent-dialogue-channel-files-20261015.json) retain the exact counters and hashes. Detailed native/model transcripts remain in ignored local verification directories.
