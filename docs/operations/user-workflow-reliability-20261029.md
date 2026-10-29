# User workflow reliability audit — 2026-10-29

This is a local repair and acceptance record. It does not declare all historical incidents resolved, announce a release, or change the public 0.2.4 installer.

## Scope and evidence limits

Reviewed every recorded external-client problem family after excluding the known workstation and its test environments: source connection, search, preview, original download/submission, local import, artifact inspection, model testing, update checks and empty searches. Counts in the operational audit are events, sometimes repeated or overlapping, not distinct bugs or people. Older reports often lack a requested date, area or asset identity; current successful examples cannot reconstruct those missing facts.

## Reproduced defects and repairs

| Area | Reproduction and change |
| --- | --- |
| Published source coverage | Maxar legacy, Vantor, GLC-FCS30D, HydroSHEDS and DE Africa could reject a valid request with no supported overlapping file as a connection error. They now return a complete empty result, with an optional explicit available-area chooser. The original area stays unchanged until the user selects an alternative. Mixed unsupported HydroSHEDS requests, invalid selectors and excessive requests remain rejected. |
| Source response contract | The native planner permits up to 64 sources, while the client accepted only 32. The response envelope now accepts the native limit; download-selection limits are unchanged. Partial connection failures remain visible and now count as failures in diagnostics despite outer HTTP success. |
| MODIS and vegetation search | Actual PC year-end MOD09A1/MOD13Q1 records declare nominal composite intervals ending in January. Accept only the exact nominal 8/16-day end or existing year-bound product end. Original catalog metadata, identifiers and DN values remain intact; an information button explains the differing declarations. Arbitrary date drift still fails. |
| Catalog transport and validation | Retry transient reads at most twice under one 30-second deadline. Cancellation, invalid parameters, authorization errors, rate limits and malformed JSON are not blindly retried. A malformed individual item no longer discards valid neighbors; the UI reports excluded records. All-invalid pages still fail explicitly. Invalid response shapes and unsafe pagination links retain the catalog-validation stage. |
| Local TIFF import | A real 5,490 × 5,490 SCL file, only 1,934,542 bytes, was rejected by a cumulative decoding-work cap intended to bound memory. Local validation now streams under a separate 1 GiB work budget. The 128 MiB chunk cap, 32 MiB input cap, dimension limit and decoding deadline remain; remote/export limits are unchanged. |
| Model and import diagnostics | Actual model-test results returning `status: failed` now emit failure rather than completion. Native model/import/inspection failures retain their operation stage; browser TIFF uploads also receive import diagnostics. Raw errors, coordinates, model replies, URLs and credentials stay outside telemetry. |
| Update checks | A transient failed automatic check previously suppressed another attempt for six hours. It now receives at most three later retries with minimum delays of 1, 5 and 15 minutes. Successful/manual checks reset the retry state. Existing user preferences, confirmation and package/signature checks remain; this does not install updates automatically. |

The two preceding local commits also repair MODIS/NDVI/EVI direct COG previews, make queue versus connection/data/validation states explicit, perform a real bounded connection read, and reduce NAIP list thumbnail size. Their detailed verification receipts remain separate from this audit.

## Actual acceptance

- Six published-source connectors succeeded in their supported footprints. Five non-overlapping Beijing requests now return empty results rather than HTTP 400, with no substituted file or automatic area change.
- Current Earth Search, PC Sentinel-2, Landsat, Sentinel-1 RTC and NOAA-21 VIIRS catalogs succeeded. Copernicus Data Space returned five current records through the actual browser catalog client in 1.68 seconds, with no rejected records. A Node-only probe timed out; independent HTTP requests and the real browser passed the same query, so that timeout is retained as a harness/environment limitation rather than a product failure. The endpoint was checked against the [official STAC API contract](https://documentation.dataspace.copernicus.eu/APIs/STAC.html). Actual year-end MODIS and vegetation catalog pages succeeded after the narrow interval repair.
- Six live map cases rendered raster pixels: Earth Search Sentinel-2, PC Sentinel-2, Landsat, GLO-30, GLO-90 and NAIP. A 99.84%-cloud Landsat control was legitimately opaque white; it was not counted as a renderer defect. A less-cloudy original rendered separately.
- NAIP latest-year search for the tested San Francisco area used two catalog requests and returned six records from 2022. Beijing used one probe and returned no coverage. No full-directory scan was performed.
- GSCloud returned 124 current public products and one SRTM90 result for the tested Beijing area. This verifies public metadata, not a new account login or a new protected download.
- Microsoft building package lookup returned no reviewed package for the tested Beijing area and two for San Francisco. These are reviewed-index results, not a live availability or building-completeness guarantee.
- Chinese filenames, UTF-8 BOM and single-Feature GeoJSON imports succeeded. GeoPackage, Shapefile ZIP, OSM XML and PBF fixture import/inspection succeeded; fixture results verify format handling, not customer files.
- The actual SCL original downloaded, retained its SHA-256 (`8ec5157fce2e960362d7b54f0f382dd09d949f57ec7dc7df8872b326b663f992`), imported with a Chinese filename and rendered in the main UI. Native pixel value 9 at column/row 1000/1000 independently matched the original. Four concurrent inspections succeeded. The managed download also reopened and rendered in Workspace.
- The saved direct DeepSeek connection passed actual text and tool-call capability probes. No conversation or geographic data was sent, no model tool was executed, and the registry/vault were unchanged.
- The real Tauri updater SDK read the public HTTPS manifest and offered 0.2.4 when compared with 0.2.3. This check-only probe downloaded no package and installed nothing. A signature being present in metadata is not a fresh package-signature verification; the separate SDK regression suite covers signed bytes and tamper rejection.

The preceding queue acceptance separately downloaded nine actual originals over a controlled TLS-preserving slow/pause connection, with independent hashes. It verifies queue/transfer behavior under that test condition, not the cause of an unidentified historical zero-byte event.

## Verification receipts

Local detailed receipts are in `.verification/all-user-issues-20261029/`; prior preview/queue receipts are in `.verification/reliability-20261029/`. They use isolated stores and block production telemetry during browser tests. No user files, source accounts, production usage data, updater feed, installers or CDN packages were modified.

Final `npm run verify` passed in full: repository/contracts/source checks, 611 frontend unit tests, 164 Agent tests, campaign/service tests, all 82 UI suites (844 tests passed, one explicitly skipped), and the production build. An earlier run exposed one outdated status-text assertion; it was corrected and the complete verification rerun passed. Native regression passed: 932 tests (927 library, four CLI-unit and one CLI integration), with 46 explicit ignores. Desktop regression passed: 61 tests, with 29 explicit ignores. Runtime Clippy with warnings denied, Rust formatting and whitespace checks passed. Build chunk-size/dynamic-import advisories remain; they do not establish a user-visible runtime defect. Historical import/inspection/model failures with missing asset/configuration details remain unconfirmed rather than being relabeled as fixed.
