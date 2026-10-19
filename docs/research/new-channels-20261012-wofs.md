# DE Africa WOfS annual summary — complete-original candidate evidence

Observed on 2026-10-12. This is independent source research and Rasterio/GDAL verification. It does **not** establish native application integration, a usable dropdown entry, or browser acceptance.

## Product and scientific meaning

WOfS annual summaries estimate the ratio of clear Landsat observations classified as wet. The published layers are `frequency` (ratio 0–1, Float32, NaN NoData), `count_wet`, and `count_clear` (observation counts, Int16, −999 NoData). The grid is 30 m in EPSG:6933. Zero frequency is a valid dry observation result; no clear observations produce NaN frequency and cannot be described as dry. This is a derived water observation product, not source reflectance, instantaneous flood extent, water depth, or percentage of physical time under water. The publisher documents a CC BY 4.0 license and limitations around mixed pixels, sediment, vegetation, and dark water. [Publisher specification](https://docs.digitalearthafrica.org/en/latest/data_specs/Landsat_WOfS_specs.html), [public distribution registry](https://registry.opendata.aws/deafrica-wofs/).

The specification page still describes an annual temporal range ending in 2023 and a shortened year-only S3 path. The live public listing contains a 2025 release for the sample tile; its real path includes `x`/`y` tile coordinates and a `--P1Y` suffix. These live findings are recorded separately from the older documentation.

## Exact published recipe

- Bucket/region: `deafrica-services` / `af-south-1`.
- Prefix: `wofs_ls_summary_annual/1-0-0/x218/y075/2025--P1Y/`.
- Actual item: [wofs_ls_summary_annual_x218y075_2025--P1Y.stac-item.json](https://deafrica-services.s3.af-south-1.amazonaws.com/wofs_ls_summary_annual/1-0-0/x218/y075/2025--P1Y/wofs_ls_summary_annual_x218y075_2025--P1Y.stac-item.json).
- Item ID: `ba1e962e-8f82-55ec-89a3-ed0b2a517cee`; dataset version `1.0.0`.
- Published annual period: `dtr:start_datetime = 2025-01-01T00:00:00.000000Z`, `dtr:end_datetime = 2025-12-31T23:59:59.999999Z`; the separate `datetime` is the first day of the year.
- WGS84 item bounds: `[36.8135240654279, -1.5051857382785072, 37.808484175304336, -0.7525296804300243]`, northeast of Nairobi.
- Original grid: `3200 × 3200`, transform `[30, 0, 3552000, 0, -30, -96000]`, projected bounds `[3552000, -192000, 3648000, -96000]`.

All three original asset hrefs are relative filenames and resolve against the actual fetched metadata directory without altering the archived publisher document. Its `self` link instead points to an S3 `.json` basename: that corresponding public HTTPS object returned **404**; the actual `.stac-item.json` returned **200**. A connector must preserve and explain this mismatch rather than silently use the nonexistent self location.

## Complete originals independently verified

| Layer | Original bytes | SHA-256 |
| --- | ---: | --- |
| `count_wet` | 355,667 | `8413faccf023730402caf9690eb93dfc80e9df24e266d4b8001c6e35716f2187` |
| `count_clear` | 4,225,860 | `422de1a7829ff1d1af3aba3d964f8263dee7e890e129535e6765a9312747981d` |
| `frequency` | 2,152,684 | `7fe06cf06a4835c55fc83ed276f74137d7113dd625e77004f3b1ea809998ab72` |

All **6,734,211 bytes** were downloaded as full published TIFFs. Each original was decoded block by block at the original grid resolution, totaling **30,720,000 pixels**, without substituting a thumbnail, overview, crop, or generated export. Each SHA-1 also matched the publisher's independently fetched checksum sidecar.

The frequency grid contains 9,384,383 finite values and 855,617 NaN values. Finite values range from 0 to 1. For every usable pixel, `frequency = count_wet / count_clear` within Float32 precision (maximum absolute difference `2.8610229518832853e-8`); no wet count exceeds a clear count. All 855,617 zero-clear-count pixels have NaN frequency.

Aligned original-value probes:

| Column / row | Wet count | Clear count | Frequency | Interpretation |
| --- | ---: | ---: | ---: | --- |
| 2021 / 617 | 18 | 18 | 1 | Positive wet-observation sample; longitude `37.4420590223389`, latitude `−0.8977550233457116` |
| 710 / 897 | 0 | 0 | NaN | No clear observations; frequency NoData, count zeros remain valid |
| 0 / 0 | 0 | 16 | 0 | Valid zero water frequency |
| 3199 / 3199 | 0 | 0 | NaN | No clear observations |

The public product covers Africa; **complete-original acceptance here is limited to the one stated 2025 sample tile**. An empty/missing tile-year must remain unavailable, and the user's area must never be replaced by this example. Additional tile/period integration still needs reviewed discovery, bounded download, source/project/job reopening, native raw-value and NaN checks, preview, and actual browser acceptance.

Full machine-readable records, original publisher JSON, live listing paths, hash receipts, grid metadata, and probes are in [new-channels-20261012-wofs.json](new-channels-20261012-wofs.json). Large original TIFFs and research scripts remain under the ignored `.verification/new-channels-wofs-20261012/` directory.
