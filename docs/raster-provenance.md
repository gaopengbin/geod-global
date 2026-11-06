# Raster files with portable source context

Completed managed GeoTIFFs can be saved together with a matching `<filename>.provenance.json`. In **My Data → Files → File details and provenance**, choose **Save raster with provenance**. The desktop action saves both files through the native file dialog. The local browser preview can prepare and download the JSON; native paired file saving is a desktop capability.

The pair retains:

- The raster filename, exact byte count and complete-file SHA-256.
- Every recorded original source Item ID, selected asset key, canonical download URL and source-file SHA-256.
- The full public STAC Item when available, including its properties, asset declarations and links. Saved custom-STAC selections use their immutable native metadata snapshot. Fixed catalog sources fetch and bind the reviewed official Item during export; this is explicitly labeled `captureStage: export`, with its retrieval time, rather than being presented as historical acquisition/download metadata.
- Actual local operations in dependency order, including selected bands, pinned source hashes, crop bounds/window/grid, masks, mosaics and scientific RGB settings where present. Parameters are data, not executable commands. Existing original-grid operations explicitly record that no local resampling or reprojection was performed.

The current format is `geod-raster-provenance/v1`; its consumer contract is [raster-provenance.json](../schemas/raster-provenance.json). The format does not add a new resampling/reprojection algorithm. Display stretches and reduced map previews are not recorded as scientific changes to the source values.

## Integrity and missing context

The application verifies the completed raster and the committed source graph before preparation. Paired export streams and independently hashes the file again, retaining its exact bytes. The JSON filename binding follows a user-selected new filename. Existing TIFF or JSON destinations are refused rather than overwritten. An incomplete copy is not published as a successful pair.

Original catalog context can be absent for older/non-STAC sources or unavailable upstream metadata. The dialog discloses the count, and each missing source has an explicit `catalog.status: unavailable` and a coarse reason. A later preparation retries unavailable context while preserving already captured snapshots. Source IDs, URLs, hashes and known processing steps remain available; the application does not invent missing publisher metadata.

Transient metadata reads receive at most two retries within one 30-second budget. Authorization, rate-limit and identity/JSON validation failures are not blindly retried. Missing-context records retain the specific timeout, HTTP, incomplete-body, decode, size-limit or identity category, available HTTP status and retry count.

Temporary SAS/signed URL queries, authentication fields and native path fields are excluded. Any credential-field redaction is counted in the JSON. The sidecar can include spatial bounds and user-entered recipe names, so review the files before sharing them. It is a portable record, not a publisher-signed scientific certificate.

## Command line

```text
geod-runtime jobs provenance --id JOB_UUID --data-dir WORKSPACE_DIRECTORY
geod-runtime jobs export --id JOB_UUID --out result.tif --data-dir WORKSPACE_DIRECTORY
```

Native output paths require exclusive direct workspace access. The loopback API exposes only bounded metadata preparation/download at `/jobs/{id}/provenance`; it does not accept arbitrary output paths. Reading an unprepared sidecar does not generate metadata or make upstream requests.

## Verified Sentinel-2 example — 2026-11-06

A fresh Earth Search Sentinel-2 L2A SCL original (`S2C_10SEG_20261030_0_L2A`, 2,864,039 bytes) was downloaded to an isolated store. A source-grid clip produced a 120 × 100 GeoTIFF. All **12,000 output samples** matched the corresponding original window in an independent Rasterio read; CRS and pixel spacing were unchanged.

The actual native paired export saved a renamed TIFF and JSON. File hashes, Item ID, original asset URL, complete STAC Item and `download → clip` parameters survived rereading. An existing destination was refused without changing its bytes. A separate native custom-STAC download retained the exact saved document SHA-256 and full original Item properties in its sidecar.

Repeating this workflow exposed an existing cache-role defect: project downloads could return a completed crop with the same original Item/asset URL. Original reuse now explicitly requires `kind: download`. A regression places a derived record before the original in cache order; repeated original requests must keep returning the original job and checksum. Existing derived files are retained.

The production renderer under the exact desktop CSP was tested at 1440 px in English/light mode and 900 px in Chinese/dark mode. The real file-details action prepared and downloaded JSON identical to the native receipt. The native core/CLI performed the paired filesystem export; the physical native Save dialog and a fresh native window were not exercised in these headless checks. No customer data, production telemetry, installer or update channel was changed.

Detailed local receipts are in `.verification/provenance-20261106/`, including `live-flow.json`, `independent-roundtrip.json`, `saved-stac-context.json` and `ui-flow.json`.

Final verification passed: 618 frontend unit tests, 164 Agent tests, 83 UI suites (847 passed, one explicitly skipped), the production build and repository/contract checks; 940 runtime tests (46 explicit ignores), 61 desktop tests (29 explicit ignores), runtime Clippy with warnings denied, formatting and whitespace checks. The final conditional retry control passed its affected UI suite after being added. Three actual exported/saved-context sidecars passed the independent JSON Schema consumer validator. Controlled tests also cover missing-context retries, tampering, renamed filenames, existing destinations and cancellation of the native Save dialog callback. An existing administrative-lookup test encountered a public upstream timeout during one parallel run; it passed separately, and the complete final runtime run passed with four test threads.
