# JAXA SGLI sea-surface temperature — 2026-10-14 batch

The `20261014` suffix identifies the work batch only. The actual live metadata probing, complete original transfer and independent decode began on **2026-10-13 UTC**, as recorded by the captured HTTP Date headers and source receipts. It is not a claim that an October 14 verification has already occurred.

The new product is `jaxa-sgli-sst`, a finite September 2026 **daytime sea-surface temperature** distribution. It is separate from the already integrated SGLI land-surface temperature and ocean chlorophyll products. One complete numerical original was downloaded and independently decoded before preparing the adapter. Native download, PNG, raw-pixel inspection and reopen acceptance then passed through a fresh software download; a local research TIFF was never injected into the runtime. Desktop or online map-preview acceptance remains separate.

## Actual source and scope

The [live JAXA Earth root catalog](https://data.earth.jaxa.jp/stac/cog/v1/catalog.json) links the [SGLI SST daytime monthly Collection](https://s3.ap-northeast-1.wasabisys.com/je-pds/cog/v1/JAXA.G-Portal_GCOM-C.SGLI_standard.L3-SST.daytime.v3_global_monthly/collection.json). Its newest child at review was September 2026. All eight actual level-1 Items were retrieved. The adapter preserves each original Item, corresponding exact Collection and relative `SST` asset declaration; it does not synthesize Items or broaden acquisition intervals.

The [official product page](https://earth.jaxa.jp/en/data/products/sea-surface-temperature/index.html) identifies GCOM-C/SGLI SST as an ocean surface-temperature measurement. The [JAXA Earth API description](https://data.earth.jaxa.jp/en/) describes the underlying SGLI product as approximately 250 m, daytime observations around local 10:30, and missing observations under clouds. This module selects **the publisher's much coarser level-1 numerical overview**, not that underlying 250 m delivery. It keeps the unified area → source → actual results → map workflow; an inland AOI is not replaced by an ocean demonstration area.

## Complete original and independent science check

[Exact NE publisher Item](https://s3.ap-northeast-1.wasabisys.com/je-pds/cog/v1/JAXA.G-Portal_GCOM-C.SGLI_standard.L3-SST.daytime.v3_global_monthly/2026-09/1/E090.00-E180.00/N00.00-N90.00.json) → [complete original `SST` TIFF](https://s3.ap-northeast-1.wasabisys.com/je-pds/cog/v1/JAXA.G-Portal_GCOM-C.SGLI_standard.L3-SST.daytime.v3_global_monthly/2026-09/1/E090.00-E180.00/E090.00-N00.00-E180.00-N90.00-SST.tiff).

| Property | Independently verified result |
| --- | --- |
| Complete transfer | 13,714,492 bytes; exact HTTP Content-Length checked |
| Local SHA-256 | `93199312ab66f7f8e48e2d5e22f48672d196aada941ed160f439351c6aa2c2a7` |
| Storage | One UInt16 band, 3,600 × 3,600, EPSG:4326 |
| Actual geographic bounds | [90, 0, 180, 90] |
| Grid spacing | 0.025° × 0.025°, about 2.8 km at the equator; not sensor resolution or coastal accuracy |
| Scalar NoData | 65535, present in original TIFF and Item; zero is not scalar NoData |
| Exact STAC calibration | temperature [degreeC] = DN × **0.0012000000569969416 − 10** |
| TIFF calibration | Embedded scale 1 / offset 0; physical calibration comes from unchanged STAC |
| Full decode | All 12,960,000 pixels; 6,516,561 numerically valid DN, 6,443,439 scalar NoData |
| Observed extrema | DN 5726–36505, exactly matching this NE Item's min/max; approximately −3.1288 to 33.8060 °C after stated calibration |
| Reserved mission flags | 65532, 65533, 65534 each occur zero times in this original |
| Internal overviews | Factors 2 and 4; TIFF declares `OVV_RESAMPLING=MODE` |
| Exact NE interval | 2026-09-01T00:25:58Z through 2026-09-30T11:03:17Z |
| Creation date | October 1, 2026; it is not an October observation interval |

The [official SGLI SST scientific declaration](https://suzaku.eorc.jaxa.jp/GCOM_C/data/update/Algorithm_SST_en.html) defines slope 0.0012 and offset −10, valid mission DN 0–65531, plus retrieval/cloud/land flags 65532–65534. The current COG Item carries only the scalar error declaration 65535 and no separate QA asset. The adapter keeps that original declaration and warns about this distinction rather than inventing missing mission masks. The exact higher precision slope in STAC is retained, and neither bytes nor stored DN are recalibrated.

Six distinct original pixel coordinates were saved for later native comparison:

| Column, row | Stored DN | Scalar NoData | Interpretation under exact calibration |
| --- | --- | --- | --- |
| 2293, 321 | 5726 | No | −3.1287996736355126 °C, observed minimum |
| 651, 2400 | 36505 | No | 33.80600208067335 °C, observed maximum |
| 0, 0 | 65535 | Yes | Missing; no temperature calculation |
| 1420, 2380 | 30952 | No | 27.142401764169335 °C, East China Sea |
| 2020, 2180 | 28912 | No | 24.694401647895575 °C, eastern Japan coastal coordinate |
| 1056, 2004 | 65535 | Yes | Beijing inland coordinate; no demonstration substitution |

“Numerically valid” means a stored value within the mission encoding range. It does **not** establish an independently validated ocean observation. The observed maximum is at an inland coordinate, demonstrating why range checks alone cannot promise a marine or coastal quality guarantee. No ocean mask, separate QA band, client averaging, clipping, reprojection or physical-value rendering was applied.

The monthly-labelled source contains real tile-specific observation intervals. A September pathname does not prove a whole-month mean, a maximum composite, or coverage at every pixel on every day. All eight intervals remain exact in the resolver. The level-1 overview's MODE distribution resampling is also separate from any temporal aggregation; the adapter asserts no unverified temporal method.

## Provenance, policy and implementation boundary

The exact original Collection uses generic ID `collection`; its Items name the full SGLI SST product ID. The product-specific validator checks the complete unchanged Collection and its exact HTTPS endpoint. It permits archiving the conflicting document without inventing an ID alias or calling it a matching verified Collection. The standalone Items lack `self`; each exact independently retrieved URL is used for revalidation.

Publisher Item and Collection license declarations are `proprietary`. The [JAXA Research Data policy](https://earth.jaxa.jp/en/data/policy/) permits fee-free use and distribution with JAXA/research-name and contributing agency credits, and requires advance notification for commercial use. No notification was sent by this research or adapter; public availability is not a Creative Commons license. Items declare no `file:checksum`. Local SHA-256, HTTP ETag and complete byte comparison are local evidence, not publisher checksum verification.

New owned files are `crates/geod-runtime/src/datasets/jaxa_more.rs`, `jaxa-more-reviewed-items.json` and `jaxa-more-reviewed-collections.json`. The module supplies `definitions`, `resolve`, `validate`, `validate_collection`, `transports` and `self_url`. Its single dataset is `jaxa-sgli-sst`, category `ocean`, layer `sst`, asset `SST`, year 2026, month 9. Unsupported dates, day/night modes, named countries/regions/seasons, layers and pyramid levels fail explicitly. AOI intersection uses half-open edges and retains the exact input area; at most eight original 90° tiles are selected. Only the NE original has complete independent per-file acceptance; the other seven are exact metadata-indexed source files.

Three module tests cover AOI/date/category boundaries and absence of substitutions; immutable scale, offset, NoData, units, license, roles, source asset and acquisition interval; and the narrowly retained original Collection conflict. Shared dispatch, source directory, frontend, Chinese localization and builds are the parent agent's responsibility.

Private evidence lives in `.verification/jaxa-more-products-20261014/`: original metadata bytes/receipts, complete TIFF/transfer receipt, all-pixel independent decode and six samples, exact JSON archive equality, saved primary documentation and a fresh-store native smoke script. The smoke requires a parent-confirmed executable hash, copies that executable, creates a new private store and downloads anew through the actual software APIs. It then checks complete bytes/hash, actual grid, six raw/NoData samples, PNG decoding and a private kernel reopen without a second transfer. No user store, model, browser, desktop, account, proxy configuration or credentials are used.

## Native acceptance

After shared registration, all three `datasets::jaxa_more` tests passed, with 783 other tests filtered out. A fresh native smoke used the fixed executable `runtime-sst.exe`, SHA-256 `01a9cfc831586abfee7a1520d7081c8387d888622590152a0c096c7e870d931e`, copied and hash-checked before startup. Its new private store was `native-run-f966c67b189146d58efc3ff37e295a97/jaxa-sgli-sst`.

The software retrieved the actual publisher Item/Collection again, created a project for the real AOI [125,30,126,31], and downloaded a fresh original. It returned 13,714,492 bytes and SHA-256 `93199312ab66f7f8e48e2d5e22f48672d196aada941ed160f439351c6aa2c2a7`, exactly matching the prior independent full transfer. Observed download polling elapsed time was 22.3674954 seconds; this is not a speed guarantee. Native inspection returned the matching 3,600 × 3,600 UInt16 EPSG:4326 grid, original bounds and NoData 65535. All six distinct raw/NoData samples matched the independent full decode. The actual 768 × 768 native PNG was decoded successfully and visually checked as a grayscale numerical overview.

The private kernel was stopped and reopened against the same new store. Snapshot, project, original hash, inspection and all six raw pixels remained unchanged, with one job and no second transfer. The helper process was stopped after acceptance; no existing desktop or user store was accessed. Full receipts, job, snapshot, native inspection, pixels, PNG, reopen result and executable hash are preserved beneath the private run directory. `native-acceptance-latest.json` points to that run. This establishes the original-download/native-inspection/reopen path, not the browser or desktop online map preview.
