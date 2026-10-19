> 2026-10-15 后续更新：下文保留当时研究状态。Nevada L2A 原包现已完整取得，新增原件身份限定的受限 ENVI BSQ 读取器并完成软件验收，详见[当前原件报告](../enmap-original-acceptance-20261015.md)。Tyrol / Groningen 与全球目录仍未接入。

# Remaining original backlog: map and spectral sources, 2026-10-14

No original Planned ID is completed by this research. GEBCO now has a separate official regional numerical-subset action; its whole-original backlog entry remains Planned. Open catalog families are not completed by one member product. Detailed live receipts and full scientific profiles are in `.verification/remaining-backlog-20261014/maps`, with the machine-readable summary next to this document.

## EnMAP: the actual current L2A examples exceed the member budget

The [official example page](https://www.enmap.org/data_tools/exampledata/) links three L2A packages. Exact strong-ETag HTTP 206 byte ranges established each complete ZIP central directory; the complete small HDR and metadata XML members were independently inflated and checked against their declared CRCs. No complete package or main spectrum was downloaded after its expanded size was known to exceed 536,870,912 bytes.

| Example | Complete compressed ZIP bytes | Complete main BSQ bytes | Original grid | Wavelength range |
|---|---:|---:|---|---|
| groningen_netherlands | 260,905,586 | 770,327,040 | 1335 × 1288 × 224 | 418.127–2445.53 nm |
| nevada_usa | 367,338,178 | 689,625,216 | 1286 × 1197 × 224 | 418.416–2445.3 nm |
| tyrol_austria | 387,343,458 | 685,862,912 | 1259 × 1216 × 224 | 418.199–2445.54 nm |

All three main spectra are publisher **ENVI BSQ**, signed Int16, little-endian, 224 bands, NoData -32768, 30 m UTM grids. Each HDR contains all 224 wavelength and FWHM values in nm, gain 0.0001 and offset 0. The member lengths exactly equal width × height × 224 × 2. The TIFF members in these packages are quicklooks or quality products; they cannot replace the main spectrum.

Tyrol was acquired 2022-06-12, Groningen 2022-08-10 and Nevada 2023-07-07. Their current V010506 processing filenames date from May 2026. Acquisition, processing and the HTTP Last-Modified date are distinct. Tyrol/Nevada use land-mode surface reflectance; Groningen uses combined processing with surface reflectance on land and underwater reflectance on water. Preserve that distinction instead of assigning one reflectance meaning to every pixel. [Official processing descriptions](https://www.enmap.org/data_tools/exampledata/).

The [EnMAP license](https://www.enmap.org/data/resources/EnMAP_Data_License.pdf) permits commercial and non-commercial use under its specific conditions, including limits on public redistribution of recoverable original pixel data. It is not a guessed CC-BY license. No credentials were registered, no L1C was substituted and no new multi-band decoder or budget exception was enabled for these oversized L2A samples.

## GEBCO: real numerical subsets, kept separate from static originals

The [official current GEBCO page](https://www.gebco.net/data-products/gridded-bathymetry-data) distinguishes elevation/bathymetry from the TID source-identifier grid. Live [CEDA GeoTIFF listing](https://data.ceda.ac.uk/bodc/gebco/global/gebco_2026/ice_surface_elevation/geotiff?json=) shows the whole ZIP at 4,241,629,269 bytes and eight actual elevation TIFFs at 933,257,436–933,257,458 bytes each. All exceed the unchanged 512 MiB static-original transfer budget. TID is not water depth.

The historical [GEBCO One Minute Grid](https://www.gebco.net/data-products/historical-data-sets) is genuinely the 2008 update of a grid first published in 2003, now superseded. CEDA exposes complete `GRIDONE_2D.nc` at 466,885,748 bytes, which fits transfer size but needs a new bounded scientific NetCDF reader; only metadata and its initial header were probed. No partial header is presented as a downloaded original, and it is not relabeled as the 2026 grid.

The supported partial action calls the [official download service](https://download.gebco.net), without email, credentials, supplied basket URLs or third-party rendering:

1. `GET /api/grids` and `/api/formats`; verify exact grid `gebco_2026_global` id 1, numerical elevation/bathymetry data-source id 1 and Data GeoTIFF format id 2.
2. `POST /api/queue` with one numerical item, bbox `[-30.1,40,-30,40.1]`, null email and no imagery or TID layer.
3. Poll `/api/queue/status/{basketId}` for the official finished state, then download the complete `/api/queue/download/{basketId}` ZIP.

The complete official ZIP contains one numerical TIFF plus the official documentation and terms. The original research request returned 421,346 bytes, SHA-256 `17988c1df26c8114762cde16e26214d570f1a9549696e3777cacd1b4d18c3b64`. The publisher-generated TIFF was 8,992 bytes, SHA-256 `86e3fceacfa0f544c7644d8c54c71acb5c73071db5944346025d381e7e58a6ce`; the 24 × 24 EPSG:4326 Int16 grid has 15 arc-second cells, NoData -32767, raw elevation -1941…-1209 m, center raw value -1705. Rasterio and tifffile agreed on every one of its 576 samples.

The independent **software-native live acceptance** created another request in a fresh private store, downloaded both complete files through the normal module, decoded and sampled them, then reopened the same store. Its per-request TIFF SHA is `b418359817cd7bfe525ec301d37c74cc82610529f229e16e321e62422075a5b0` and ZIP SHA is `e12b32a39bb6c2a87a3b05eb0d6241acce03dd86694bae19bed5cd1ee43509a7`. All 576 grid samples and nine native pixel reads match the independent reference. The official TIFF `NC_GLOBAL#history` embeds generation time and basket identity, so cross-request SHA equality is not required and that original metadata is never rewritten. The first obsolete cross-request SHA assertion failed; the complete first native download remains preserved separately. The corrected fresh acceptance passed and records `researchDataCopiedIntoStore=false`.

The actual native PNG is 24 × 24, with 576 opaque pixels, 0 transparent pixels and 168 grayscale values. It is a first-band browsing view, not an alternative scientific raster.

The module retains exact user request, official submitted request, queue response, grids/formats responses, whole ZIP and unmodified TIFF with separate SHA receipts. Every read/export rechecks custody. ZIP checks reject encryption, unsupported compression, unsafe/duplicate names, Unix devices/links, interval overlap, CRC errors and cumulative expansion over 32 MiB. Files use exclusive creation and sync before atomic managed directory publication. GET/list stream hashes and CRCs rather than clone whole archives. Subset limits remain 32 MiB archive and cumulative expansion, 10,000 pixels per axis, 128 MiB full-chunk decode and a three-minute total generation/transfer/verification timeout. Ordinary original-reader ceilings are unchanged.

Explore uses the existing AOI → source → generated result list → explicit multi-select map loading, with up to four layers, visibility, opacity and removal. Changing the AOI clears selections and ignores old in-flight previews. `libraryOnly` reopens saved regions without new queue requests or stale Explore AOI. GeoTIFF and whole official ZIP exports are separate, both verified; installed desktop exports use their own manager/store through native save commands.

GEBCO places the grid in the public domain subject to its [terms, attribution and limitations](https://www.gebco.net/data-products/gridded-bathymetry-data). Elevations generally assume mean sea level, but some shallow-water inputs use another vertical datum. 2026 is a grid release, not one acquisition date; successful decoding does not establish navigation suitability or local survey accuracy.

## Remaining open catalog families

| Original ID | Live catalog | Concrete tested scope | Result and honest boundary |
|---|---:|---|---|
| earth-search-more | 9 collections | Real NAIP Item `ca_m_3712256_nw_10_060_20220519`, 2022 RGBIR 60 cm | Query returned 200 with the original S3 href; anonymous original request returned 403. Existing C1 L2A action remains finite/partial; NAIP was not silently remapped to another product or marked available. |
| planetary-more | 139 collections | NOAA OISST v2.1 Item `oisst-avhrr-v02r01.20200901`, daily SST COG | Complete canonical platform COG downloaded: 1,096,226 bytes, SHA `47b19bb75c95cdd302b1372a4d6a5a910c3e7213ccc013e6fee5f82f48646bfa`; 1440 × 720 Int16 EPSG:4326, NoData -999. All samples agree between rasterio/tifffile. Raw examples -178, 2538, 1543, 1558 are preserved; publisher Celsius scale 0.009999999776482582 is declared, not silently applied. Anonymous public SAS transport stayed transient and was not stored. This is a published COG derivative, not the NOAA source NetCDF, and no adapter or parent-family completion is claimed. |
| copernicus-more | 10 collections | CLMS daily burnt-area V3.1 global COG | Collection declares S3/OpenID Connect access. Its Item-list request actually returned 200 **HTML Request Rejected**, not JSON. No original download, credentials, account creation or access-control bypass occurred; no blanket platform-ready claim. |

Canonical official catalogs: [Earth Search](https://earth-search.aws.element84.com/v1/collections), [Planetary Computer](https://planetarycomputer.microsoft.com/api/stac/v1/collections), [Copernicus Data Space](https://stac.dataspace.copernicus.eu/v1/collections). A collection count or one finite derivative does not establish every platform product, year, format or authorization path.

For the OISST candidate, preserve the platform Collection's `proprietary` license field alongside its linked [NOAA CDR Use Agreement](https://www.ncei.noaa.gov/pub/data/sds/cdr/CDRs/Sea_Surface_Temperature_Optimum_Interpolation/UseAgreement_01B-09.pdf). The NOAA policy makes CDR data publicly available without restrictions on use and requests appropriate scientific credit; that does not justify silently replacing the original Collection field with a guessed license. The [NOAA product documentation](https://www.ncei.noaa.gov/products/climate-data-records/sea-surface-temperature-optimum-interpolation) describes blended, bias-adjusted satellite, ship and buoy observations with interpolated gaps, not unprocessed AVHRR measurements.
