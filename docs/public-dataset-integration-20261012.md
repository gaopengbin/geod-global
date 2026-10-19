# Public numeric dataset connector acceptance — 2026-10-12

All six representative source flows passed the final native binary: reviewed connection, saved source/project, complete original, independent full decode, native georeferencing and preview, raw pixel/NoData readback, and project/source reopening. The six complete numerical originals total **83,405,694 bytes**. Two real compatibility issues discovered during testing were fixed and verified against the retained original files; the final recheck started no replacement transfers. Endpoint research and HEAD responses alone are not download acceptance.

The shared path is `dataset catalog → scope/version/layer review → actual metadata connection → immutable source selection → saved project → explicit original download → preview/raw pixel inspection → reopen project`. These products are numeric classifications, estimates, annual composites and relief grids; they are not interchangeable with current sensor scenes.

## Actual representative samples

| Dataset family | Whole original bytes / time | Dimensions | Meaning and boundary |
| --- | --- | --- | --- |
| ESA WorldCover | 20,245,485 / 6.125 s | 36,000 × 36,000 × 1 | 2021 v200 N78E015, numeric 10 m land-cover classes; 2020 and 2021 use different algorithms |
| WorldCover annual composites | 2,829,765 / 4.531 s | 6,000 × 6,000 × 2 | 2021 SWIR N78E010, numerical annual composites at 20 m; not an original 10 m scene |
| WorldPop | 156,803 / 2.079 s | 214 × 245 × 1 | Niue 2020 Global1 modelled population count per cell; not Global2 or a census |
| JRC Global Surface Water | 18,590,545 / 22.047 s | 40,000 × 40,000 × 1 | v1.5 occurrence 110E/50N, long-term water occurrence over 1984–2024 |
| GLAD Hansen Global Forest Change | 16,387,378 / 18.500 s | 40,000 × 40,000 × 1 | 2025 v1.13 lossyear 50N/110E; 0 means no detected loss, not missing data |
| NOAA ETOPO | 25,195,718 / 78.000 s | 3,600 × 3,600 × 1 | 2022 v1 15″ surface N45E000, land/sea relief and ice surface relative to EGM2008 |

Transfer times are one measurement from this machine/network and include the native queue/settlement observation; they are not service guarantees or controlled proxy-node comparisons. All six settled originals were retained for the final verifier run; their original transfer times and file hashes were preserved, not represented as new zero-cost downloads. The [machine-readable receipt](public-dataset-integration-20261012.json) records each complete source, request, native project/job, scientific block readback and original binary checksum.

## Compatibility findings from the real run

- JRC's original/overview TIFFs use ZSTD compression, while the initial native dependency had that codec disabled. The full file and independent GDAL decode passed; native preview explicitly reported unsupported ZSTD. The final binary enables the codec and passed native preview and original-index pixel comparisons; palette indices remain numerical classes, not expanded RGB values.
- ETOPO stores horizontal EPSG:4326 and vertical EPSG:3855 together; GDAL identifies the original compound CRS as EPSG:9518. The initial conservative native reader omitted georeferencing. The final binary exposes the verified horizontal grid and separately retains `verticalCrs`, original ModelPixelScale `[1/240,1/240,1]` and ModelTiepoint `[0,0,0,0,45,0]`. Original EGM2008 heights, CRS tags and SHA-256 remain unchanged. Native inspection does not infer a compound EPSG identifier.
- JRC and GLAD's actual tested grid is 0.00025° per pixel (0.9 arc-second), with nominal approximately 30 m resolution. The current catalog and newly created schema v2 source proofs now declare that exact grid. Six original schema v1 snapshots and settled download job records were retained unchanged in the final report and successfully restored with the corrected binary; only the exact historical JRC/GLAD resolution strings receive read compatibility, while the other declaration fields remain strictly validated. No source evidence was rewritten and no resampling was done.
- “Valid pixels” in the independent receipt means finite pixels after the TIFF's declared mask/NoData, not scientific QA or valid satellite coverage. In particular, GLAD zero loss remains a valid code; additional datamask interpretation is a separate scientific task.

## Metadata breadth checked separately

All **17 current layer options** across the six preset families passed live native TIFF header and immutable snapshot connection checks. The 2020 WorldCover map and four annual-composite variants also passed, producing **22 successful metadata checks** with no new original download jobs. This is representative endpoint/header acceptance, not complete-file acceptance of 22 products, all countries, every tile or every year. The full-file result remains six originals. These metadata checks used the initial binary; they exercise the same reviewed resolver/source connection path and were independent of the later ZSTD/compound-CRS inspection fixes.

One additional JRC occurrence metadata-only resolve/connect check passed with the final binary, verified proof schema v2 and `approximately 30 m (0.00025 degree grid)`, and confirmed an empty download-job list before and after connection. This separate check does not add a seventh original download. The final six-original recheck binary SHA-256 is `3d6f2a3c5f9f6e4dd802722106830833d1097dd66a90416ecddc2eb6ffae1f4a`; all original checksums and historical source snapshot hashes remained unchanged.

## Evidence requirements

The isolated native run verified that discovery created no download job, that saved source references started explicit transfers, and that final managed files matched their complete lengths and SHA-256. Rasterio decoded every block of all seven bands (4,581,012,430 samples) and compared 30 native raw pixel coordinates / 35 band values and NoData flags. Six preview PNGs parsed successfully. Reopening the private store retained all six project selections, source references, successful checksums and repeatable raw pixel readback.

Native generic previews are grayscale stretches of raw values. No thematic palette, reflectance calibration, population conversion, radar correction, scientific accuracy certification or desktop-window acceptance is inferred from those previews. SHA-256 establishes file integrity, not upstream authenticity or the accuracy of the scientific model. Existing ZIP packaging supports derived rasters, not these generic original sources; no original-source ZIP is claimed.

## Primary references and verified endpoint research

- [ESA WorldCover data access](https://esa-worldcover.org/en/data-access): CC BY 4.0; direct public AWS map tiles and separate S1/S2 annual composites. The actual S2 file suffix is `S2RGBNIR`; S1 products use the `vvvhratio` prefix and `S1VVVHratio` suffix. Published scale/offset/NoData conventions must be preserved.
- [WorldPop product catalog](https://hub.worldpop.org/project/categories?id=3): the 2000–2020 Global1 archive is separate from [Global2 R2025A](https://data.worldpop.org/repo/prj/Global_2015_2030/R2025A/doc/Global2_Release_Statement_R2025A_v1.pdf). The representative file is the complete historical Niue country count raster; it does not establish all country/product access or current releases.
- [JRC Global Surface Water download](https://global-surface-water.appspot.com/download): current v1.5 includes 1984–2024 long-term occurrence. The provider's live HTML gives the CloudFerro distribution root and `_v1_5_2024.tif` filenames. Several history and seasonality layers have different temporal extents, and the published monthly-recurrence issue remains disclosed. Rendered WMS imagery is not numeric analysis data.
- [GLAD Hansen GFC direct download](https://storage.googleapis.com/earthenginepartners-hansen/GFC-2025-v1.13/download.html): anonymous GCS originals and CC BY 4.0. Treecover2000 is a 2000 baseline, gain is 2000–2012, and lossyear extends through 2025. The author cautions about interannual algorithm consistency and estimating definitive forest-loss area from pixel counts.
- [NOAA ETOPO](https://www.ncei.noaa.gov/products/etopo-global-relief-model) and [official ISO metadata](https://data.noaa.gov/metaview/page?header=none&view=getDataView&xml=NOAA/NESDIS/NGDC/MGG/DEM//iso/xml/etopo_2022.xml): 15″ GeoTIFF distribution, CC0-1.0, citation and EGM2008 vertical reference. This is a fused relief model, not raw satellite imagery, and is not intended for navigation.

## Software and interface checks

The repository verification command passed frontend and Agent unit tests, UI integration tests, dependency/desktop permission checks and the production frontend build. Formatting, all native-runtime tests (632 passed, 15 opt-in ignored), desktop tests (56 passed, 27 opt-in ignored), and workspace static analysis with warnings treated as errors passed. Ignored actual-model tests were not represented as model acceptance.

An actual browser flow against an isolated live native runtime selected WorldPop Niue 2020, resolved and checked the published file, explicitly saved a project and queued its original download, then opened its real map preview and read column 107 / row 122 as `0.02245340682566166`. The completed file was 156,803 bytes with SHA-256 `6cf1f3e45799a10d8803a5cf94defd9903d3adabc877daef5b0b157d9214d7cc`. Reloading reopened the saved source. The product-connector and original-inspector dialogs were visually checked at a 390 × 844 viewport. The Explore **Add data source → Public dataset connectors** entry opened the same reviewed product catalog. The browser adapter does not run the desktop Agent; no actual-model or native-window visual acceptance is inferred.

Desktop external-link validation now permits the catalog's exact reviewed documentation and licence URLs, while rejecting altered paths and credentials. No broad additional storage-host whitelist was introduced.

## Further channels retained for follow-up

[OpenLandMap](https://docs.openlandmap.org/020-stac.html) exposes a live public static STAC with 104 child collections in this machine's metadata readback. Global raster assets can be very large and licenses vary by collection. [Copernicus DEM on AWS](https://registry.opendata.aws/copernicus-dem/) offers public 2021 GLO-30/GLO-90 DSM files; GLO-30 has omitted-country holes, and sea areas have no tiles. [CBERS on AWS](https://registry.opendata.aws/cbers/) uses the current `brazil-eosats` bucket and CC BY-SA 3.0, with INPE-received coverage. [GHSL](https://human-settlement.emergency.copernicus.eu/download.php) mostly delivers numeric grids in ZIP archives and varies by epoch, CRS and resolution. These metadata findings do not establish full-file or software-flow acceptance in this six-family run.

SoilGrids remains a distinct WCS integration task: the public service responds, but its native Homolosine grid must be supported correctly by the product before a software subset workflow can be called accepted. A reprojected external sample does not substitute for that product capability.
