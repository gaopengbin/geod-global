# Four JAXA land and atmosphere products — 14 October 2026

This batch adds four distinct formally published products, not additional years, months, bands or aliases of an existing product: SGLI terrestrial ET, its separate dimensionless ET index, SGLI aerosol optical depth (AROT), and the independent Terra+Aqua MODIS JASMES TAUA optical-depth product. Only the fixed releases and COG levels below are enabled. No commercial imagery procurement, paid Earth Engine project, model credential or account token is used.

## Live publisher discovery and scientific identity

The [JAXA Earth root catalog](https://data.earth.jaxa.jp/stac/cog/v1/catalog.json) was read live on 14 October 2026. Each selected product's actual Collection, released month, COG-level catalog, longitude catalogs and all eight Item documents were retrieved and preserved unchanged in `jaxa-land-batch-reviewed-items.json` and `jaxa-land-batch-reviewed-collections.json`. Four Collections and 32 actual Items are archived; Items are not synthesized.

The [JAXA-authored ET algorithm paper](https://www.jstage.jst.go.jp/article/agrmet/82/1/82_D-25-00022/_html/-char/en) describes ET as a temperature/weather-derived terrestrial water-flux estimate, and ETindex as its dimensionless evaporative-efficiency index. The reference algorithm uses a calibrated maximum index of 1.23, so an index above one is scientifically distinct from a bounded percentage and must not be clamped to one. That paper's underlying 250 m algorithm grid does not establish the resolution of this released 0.05° TIFF.

[JAXA's aerosol product description](https://earth.jaxa.jp/en/data/products/aerosol/index.html) and [the JASMES data list](https://www.eorc.jaxa.jp/JASMES/datalist.html) were read live. SGLI AROT and Terra+Aqua MODIS TAUA/T500 are independently published aerosol-column optical-depth products. They are not surface particulate concentration or PM2.5 estimates. Their 500 nm identification comes from the unchanged publisher titles; missing physical-unit fields remain missing in source metadata.

| Software product | Exact publisher product | Fixed request | Original measurement asset | Actual grid |
|---|---|---|---|---|
| `jaxa-sgli-et` | `JAXA.JASMES_GCOM-C.SGLI_ET.v1_global_monthly` | April 2026, level 1, `et` | `ET` | 1,800 × 1,800, 0.05°, level tagged `raster` |
| `jaxa-sgli-et-index` | `JAXA.JASMES_GCOM-C.SGLI_ETidx.v1_global_monthly` | April 2026, level 1, `et-index` | `ETidx` | 1,800 × 1,800, 0.05°, level tagged `raster` |
| `jaxa-sgli-aerosol` | `JAXA.G-Portal_GCOM-C.SGLI_standard.L3-AROT.daytime.v3_global_monthly` | September 2026, level 1, `aod` | `AROT` | 3,600 × 3,600, 0.025°, level tagged `overview` |
| `jaxa-modis-aerosol` | `JAXA.JASMES_Terra.MODIS-Aqua.MODIS_taua.v811_global_monthly` | September 2026, level 1, `aod` | `T500` | 1,800 × 1,800, 0.05°, level tagged `raster` |

All reviewed NE originals cover `[90, 0, 180, 90]` in EPSG:4326, contain one UInt16 band, are DEFLATE-compressed tiled COGs, and embed raw scale 1, offset 0, NoData 65535 and MODE overview resampling. A degree grid is not constant metres, sensor sampling resolution, or local scientific accuracy. Complete tiles are preserved; an AOI selects intersecting files without cropping them.

## Science and metadata conflicts retained

| Product | Exact STAC DN slope / offset | Original physical-unit field | NE decoded valid values | Relevant limitation |
|---|---|---|---|---|
| ET | 0.0004615384677890688 / −5 | `mm/day` | DN 10833–26165 | Collection title says `1 month total evapotranspiration [mm/day]`; a sum in mm and a rate in mm/day are different. The adapter preserves that unresolved aggregation wording and applies neither day multiplication nor automatic calibration. |
| ETidx | 0.00009999999747378752 / 0 | `N/A` | DN 0–12299 | 21,574 valid zero pixels remain valid. Maximum physical index is approximately 1.2299; no [0,1] clamp or percentage conversion. |
| SGLI AROT | 0.00009999999747378752 / 0 | `NA` | DN 11–50000 | Declared NE lower bound 10 differs from decoded minimum 11. Bounds are retained, rather than treated as scientific quality masks. |
| MODIS T500 | 0.001 / 0 | `null` | DN 1–5404 | Declared upper bound 9999 differs from decoded maximum 5404. Do not substitute the SGLI scaling. |

ET and ETidx Items declare April 1 00:00:00.000000Z through April 30 23:59:59.999999Z; MODIS T500 declares September 1 through September 30 with the same timestamp precision. AROT declares September 1 00:24:46.800000Z through September 30 23:40:19.200000Z. The exact intervals are retained. Creation dates do not replace acquisition dates, and a monthly label is not independent verification of temporal averaging.

ET/ETidx are model-derived and can use assumptions when clear-sky observations are unavailable. No separate model QA layer is supplied by the indexed Items. Both aerosol Items similarly supply a scalar NoData/error declaration, without a separate aerosol QA/cloud-mask asset. This adapter does not invent the source mission's missing masks or claim every non-NoData pixel is a scientifically valid observation.

ET and ETidx use actual product IDs in their Collections and STAC 1.1.0/projection v2 declarations. The two aerosol Collections use generic ID `collection`, while their Items name their full products. The exact original Collection conflict is archived without an invented alias or a matching-Collection verification claim. Every Item lacks a self link; only its exact retrieved HTTPS URL is used for revalidation.

## Independent full-file evidence

Four fresh complete HTTP 200 transfers were performed on 14 October 2026, and all pixels were decoded independently with Rasterio 1.3.9. Six exact source pixel samples per product include valid minimum, valid maximum, ordinary valid observations and scalar NoData. The ETidx minimum sample proves zero is valid.

| Product | Bytes | Local SHA-256 | All decoded pixels | Valid / NoData |
|---|---:|---|---:|---:|
| ET | 2,001,784 | `e240a6cc7ba4900a4125a465e8eac609535547d6d53d520437a42f27d88ed482` | 3,240,000 | 1,204,319 / 2,035,681 |
| ETidx | 1,277,652 | `7a1bb26dd546645a760fbf52d4638ab8c591c3bb4099ff19a96da4313c5be1ac` | 3,240,000 | 1,204,319 / 2,035,681 |
| AROT | 5,308,270 | `a82a986f50dbdbac9b24416e07c5b4c09ec5a5cfbf1286e4468f15402619e040` | 12,960,000 | 10,414,573 / 2,545,427 |
| MODIS T500 | 4,707,747 | `02309ad2b7f3e2bdf6ef7c1bc776fa7d85e8e6e22176a7d5026322378db7f4c6` | 3,240,000 | 2,840,126 / 399,874 |

Local hashes and observed HTTP ETags establish transfer receipts, not a publisher checksum guarantee: these Items declare no file checksum. Private evidence lives under `.verification/ten-products-20261014/jaxa-land/`, with exact metadata HTTP receipts, complete original files, decoded grids/tags, all-pixel counts and 24 raw samples. Only the NE original of each product was fully transferred; other seven tiles retain actual reviewed metadata and are not individually claimed download-verified.

## Adapter and native acceptance

`jaxa_land_batch.rs` defines separate dataset identities, finite releases, immutable original scientific declarations and layer mappings. Request mismatches and changes to asset href, slope, NoData, roles, interval, license, Collection, bbox or self-link fail explicitly. Public HTTPS relative assets use ordinary existing transport safeguards; there is no private-bucket exception, retry weakening or file-size bypass.

The first frozen binary (`24763d45babbee638f4b7a175c9310e171be6b4082e50ccf90136255653ef014`) completed ET's native full download, six raw/NoData pixels, PNG and restart in a new private store. ETidx then exposed a real source-connection-name length failure before any download job: the descriptive title plus tile name exceeded the existing 80-character limit. The adapter now keeps the descriptive display title but uses a shorter native connection name, with a regression assertion. This failed first run remains preserved under `native-run-505137e8615d4444a5e3b58b007c9ba0`; it is not rewritten as a success. Final four-product acceptance passed using corrected frozen binary `4b6ca02381a99c6e4c8d9176eb7e85077ba866bc981c2ef688a54090f70cf2a2`, in four new private stores under `native-run-afb319e5034b46fd92b4da3c08665c57`. Each product used the actual dedicated `/datasets/connect` API, fresh selected-file download and the unchanged source bytes; no research TIFF was injected.

| Product | Fresh download polling time | Native snapshot | Project | Job |
|---|---:|---|---|---|
| `jaxa-sgli-et` | 1.203 s | `b84004dba02380ebf4180dc241a3b1cfc1cc88eccda2fb6a85a06788aa74f6dc` | `b4473251-6665-498d-8156-205d67083f4d` | `d502a451-e582-4b67-88d5-5a37848fe88f` |
| `jaxa-sgli-et-index` | 1.516 s | `cc3682fd8ef93671c4299ba72cdc119c253bfb0e2febce5a674137f9ee040cf2` | `96f8ac7c-3109-444a-84f9-5ac27569b621` | `e575903a-281b-42f7-bc8d-0c8c52e7ff1f` |
| `jaxa-sgli-aerosol` | 4.531 s | `cfd2e8d058bf1ae50a79a4c1e9db72cf44f1b1eaeaee73944fd86f3eda31a7c7` | `ccdf1035-1be3-49fa-8b18-419700c2a830` | `48adb3bf-7318-4ff9-adb8-aa093c970e6b` |
| `jaxa-modis-aerosol` | 3.625 s | `0a053d8ecb9c047eb892029ecc32baed2c8a8d3df7c0cb894c4c3e3c5879e75f` | `293c512e-f0ea-4c84-80e4-01d79f04a8f2` | `2dcecc84-a140-4477-a37d-aaf5569247a4` |

All four native grid/NoData checks, all 24 raw sample comparisons, and four decoded 768 × 768 PNG previews passed. After independently restarting each private runtime, the same snapshot, source declarations, selected project, original hash, pixel values and inspection result reopened unchanged; each store still contained exactly one job and no retransfer. All private helper processes were stopped and verified absent afterward. The first ETidx connection-name failure remains preserved. The declared stored DN/physical-unit conflicts and source-license limits remain in place. The unit/build check executed by the coordinating agent passed 105 targeted dataset tests on the final frozen implementation; this is not a claim that the full runtime test suite was executed here.


Research original files are never injected into the runtime's store. The native test separately connects through the dedicated product API, creates a selected-file project, freshly transfers the full original, matches its independent bytes/hash, inspects the native grid/PNG and six raw samples, then restarts and reopens unchanged data with a single job and no retransfer. Frontend visual acceptance is a separate root-agent check.

## License

All four original Items and Collections declare `proprietary`. The [JAXA research-data policy](https://earth.jaxa.jp/en/data/policy/) retains research/source and contributing-agency credits and calls for advance notification of commercial use. Public delivery does not grant a Creative Commons raster license, and the ET algorithm paper's CC BY license does not replace these raster terms. No commercial-use notification was submitted. Preserve JAXA/EORC, the named research product, original sensor, NASA attribution for MODIS-based data, source Items, exact acquisition intervals, and access dates.
