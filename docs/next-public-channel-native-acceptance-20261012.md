# Four additional public-product native acceptance cases — 2026-10-12

The [structured native report](next-public-channel-native-acceptance-20261012.json) records four passed cases: USGS 3DEP Current 1 arc-second elevation, CHELSA climate moisture index, JAXA AMSR2 soil moisture and JAXA SGLI land-surface temperature. Each case retains a real published numerical GeoTIFF, completed native download job, matching independent original hash, original-grid preview, raw-value comparison and reopened project/source. These are four distinct data products; they are not four additional satellite platforms, four optical imagery sources or acceptance of every product offered by these publishers.

The tested originals total **269,362,128 bytes** and **928,832,544 independently decoded grid pixels**, with **33 native raw-pixel coordinates** compared against Rasterio 1.3.9 / GDAL 3.6.4. No model calls, credentials or Earth Engine computation requests were used. The isolated acceptance runtime was stopped after completion. Browser interaction, a visible native window, an installed-app upgrade and production deployment are separate acceptance stages; this report does not claim any of those passed.

## Binary and persistence scope

The final inspection/reopen binary SHA-256 is `b63daf248f7c042d7109594d7df944b61fb6e576342da1f09b21026dc2c92580`. The preceding download binary was `6653efdfc6f12781c9c57ef2bb9411cc03b356c991f9627081d7b58fe8d5a766`.

All four cases explicitly record `settledTransferReusedForFinalInspection:true`. Their real completed transfers were retained, then the final binary reopened the same isolated store and repeated original-file hashing, independent full-grid decoding, native preview/raw inspection and project/source recovery. The download times below are the observed completed native jobs, not four additional downloads performed by the final binary. This verifies recovery of this earlier binary's four persisted sources and jobs; it does not prove compatibility with every historical or user-owned store.

The final report marks `originalSha256Verified`, `reopenedOriginalPreviewVerified` and `reopenedProjectAndSourceVerified` true for every case. Original publisher metadata bytes and hashes are archived where actual publisher documents exist. A successful metadata connection remains separate from a successful transfer and useful scientific coverage.

A separate user-preview-store compatibility check backed up the existing store, replaced its executable only when no jobs were active, and read back all 12 pre-existing finished jobs with zero active jobs. That verifies loading of those older saved job records. It does not claim all 12 old originals were decoded again. Subsequent browser-driven downloads into that preview store are a separate active run and are not merged into this four-original isolated native result.

## Measured originals and download times

| Actual product / original | Complete bytes | Original dimensions | Full pixels decoded | Observed native download seconds | Native raw coordinates |
| --- | ---: | --- | ---: | ---: | ---: |
| USGS `USGS_1_n41w074_20260917.tif` | 20,582,281 | 3,612 × 3,612 | 13,046,544 | 2.890 | 9 |
| CHELSA `CHELSA_cmi_mean_1981-2010_V.2.1.tif` | 244,959,851 | 43,200 × 20,880 | 902,016,000 | 334.156 | 7 |
| JAXA AMSR2 `E090.00-N00.00-E180.00-N90.00-SMC.tiff` | 140,042 | 900 × 900 | 810,000 | 1.250 | 9 |
| JAXA SGLI `E090.00-N00.00-E180.00-N90.00-LST.tiff` | 3,679,954 | 3,600 × 3,600 | 12,960,000 | 1.657 | 8 |

CHELSA's one complete quasi-global original took about 5 minutes 34 seconds in this run. The JAXA files are much smaller. These observations combine actual file sizes, route conditions and runtime job timing; they are not controlled network benchmarks or a guarantee of speed in other countries or proxy nodes. No full original was replaced with a thumbnail, cropped subset or differently dated product to improve the result.

## Original hashes and publisher integrity declarations

All four native original hashes match separately downloaded independent references:

| Product | Actual original SHA-256 |
| --- | --- |
| USGS 3DEP | `4f23a1a0801a74b1d1e9b4e83c09a41ee371010cbdca7bff0a1a92c05f0dc3dd` |
| CHELSA CMI | `5aa9f5f1aa14f8671807a73181c13aa55217ab1a653081111e691fa89c6cd283` |
| JAXA AMSR2 SMC | `fff9b57eafe2f0d312226bcab9c71c1e435fa8fc732b8f1dabb413f7cb0be3f1` |
| JAXA SGLI LST | `8172496fa42359cc8ba0f38ab79a2eb586bfaec9085a9d0839f54f2bd41093a5` |

The report's asset-integrity fields do not declare a publisher SHA-256 for these four files. Independent/local SHA-256 agreement is therefore not presented as publisher checksum verification. USGS's actual TNM record separately declares `sizeInBytes:20582281`, matching the downloaded original. The two JAXA item assets declare neither `file:checksum` nor `file:size`; independently observed simple HTTP ETag MD5 matches remain a separate receipt. No declared conflicting checksum was ignored.

## Product-specific science, time and masks

### USGS 3DEP Current 1 arc-second elevation

The requested small New York AOI queried the actual public [National Map catalog](https://tnmaccess.nationalmap.gov/api/v1/products?datasets=National+Elevation+Dataset+%28NED%29+1+arc-second+Current&bbox=-73.994%2C40.71%2C-73.99%2C40.714&max=100&offset=0&outputFormat=JSON). The selected ScienceBase source ID is `6aac9d7f1ba49b6a17baded8`; its record points to the exact [versioned original TIFF](https://prd-tnm.s3.amazonaws.com/StagedProducts/Elevation/1/TIFF/historical/n41w074/USGS_1_n41w074_20260917.tif). An archived raw catalog response and selected record are retained. This is a direct-raster client binding, not a fabricated publisher STAC item.

The file is tiled Float32, EPSG:4269 / NAD83 geographic, PixelIsArea, with the original overlapping border and approximately one-arc-second grid. It is not silently relabeled WGS84 or cropped to the AOI. In this CONUS product, heights are metres relative to the publisher's NAVD88 declaration; vertical references may differ elsewhere. Native file inspection does not invent a vertical CRS from horizontal GeoKeys.

The publisher release date `2026-09-17` is a publication field, not a terrain survey or satellite-acquisition timestamp. Zero and negative terrain values remain valid: native comparisons include 0, -0.016024990007281303 and 13.575018882751465. Original NoData -999999 was also checked. Independent decoding found 13,024,920 non-NoData pixels and 21,624 NoData pixels. License is the [USGS 3DEP public-domain declaration](https://www.usgs.gov/3d-elevation-program/about-3dep-products-services). The separate 1 m products and LiDAR are outside this connector's scope.

### CHELSA climate moisture index

The tested [complete CMI mean original](https://os.zhdk.cloud.switch.ch/chelsav2/GLOBAL/climatologies/1981-2010/bio/CHELSA_cmi_mean_1981-2010_V.2.1.tif) is fixed V2.1, **1981–2010 climate normal**. It has no year/month picker, current-weather meaning or alternative-variable fallback. Scientific interpretation follows the authors' [CHELSA-BIOCLIM+ paper](https://doi.org/10.5194/essd-14-5573-2022), which defines CMI as precipitation minus potential evapotranspiration, in kg m^-2 month^-1. The current summary-page ratio wording conflicts with that definition and is disclosed rather than reinterpreting these signed samples as a dimensionless ratio.

The original is stripped signed Int16, EPSG:4326, PixelIsArea, 30 arc-seconds. Its embedded scale is 0.1, offset 0 and NoData 32767. Native raw reads remain signed integers: Beijing column 35568 / row 5291 is -323, corresponding to -32.3 after the documented scale. A negative value is a modeled climatic water deficit, not missing data or current drought severity. The original small fractional-cell outer edges beyond -180°/-90° remain in the file; only the legal client query footprint is bounded. Original grid, transform, bytes and click-to-pixel mapping were not rewritten.

Independent decoding found 224,368,589 non-NoData pixels and 677,647,411 NoData pixels; raw valid range -3349 to 17393. An ocean/missing sample of 32767 was verified separately. The author's [exact climatologies page](https://www.chelsa-climate.org/datasets/chelsa_climatologies) declares CC0 1.0; this remains a client-referenced license declaration, not an invented STAC license field. Other variables, scenarios and malformed unsigned products are not enabled by this case.

### JAXA AMSR2 nighttime monthly soil moisture

The exact December 2025 [publisher Item](https://s3.ap-northeast-1.wasabisys.com/je-pds/cog/v1/JAXA.G-Portal_GCOM-W.AMSR2_standard.L3-SMC.nighttime.v3_global_monthly/2025-12/1/E090.00-E180.00/N00.00-N90.00.json) declares one `SMC` asset. This is JAXA Earth's formal converted numerical COG distribution, not mission-original HDF. Request layer `smc` selects only the reviewed level-1 nighttime monthly product; the map AOI chooses intersecting 90° tiles without substituting a demonstration region or another month.

The original Int16 grid is EPSG:4326 with 0.1° spacing. That spacing does not establish sensor footprint, independent local detail or a fixed soil-depth interval. Valid DN 0–600 × 0.1 gives volumetric percent. The file itself retains scale 1, offset 0 and no embedded unit; raw inspection does not automatically apply the scientific conversion.

The precise source interval is `2025-12-01T00:00:00.885000Z` to `2025-12-31T23:56:41.333000Z`. Original NoData is -32768. Native checks confirmed Beijing DN 31 / scalar NoData false, ocean DN -32768 / NoData true, and error DN -32767 / **scalar NoData false**. Seven original error codes -32767 through -32761 are declared; none are converted into moisture or silently made NoData. The generic independent non-NoData count is 228,330, including 7,200 error-coded pixels. The separate scientific count is 221,130 valid moisture DN, so those two counts must not be conflated. A raw grayscale stretch includes the preserved error codes and is not a calibrated soil-moisture map.

### JAXA SGLI daytime monthly land-surface temperature

The exact December 2025 [publisher Item](https://s3.ap-northeast-1.wasabisys.com/je-pds/cog/v1/JAXA.G-Portal_GCOM-C.SGLI_standard.L3-LST.daytime.v3_global_monthly/2025-12/1/E090.00-E180.00/N00.00-N90.00.json) declares one `LST` asset. The retained UInt16 EPSG:4326 COG grid has 0.025° spacing, while the source monthly map is approximately 5 km. Neither the grid nor its preview implies additional independent sensor resolution. This is daytime land-surface temperature, not two-metre air temperature or a climate normal.

Valid DN 11203–16860 × `0.019999999552965164` gives Kelvin, offset zero. Raw DN 13811 at Beijing column 1056 / row 2000 corresponds to approximately 276.219993826 K, while scalar NoData/error 65535 is never interpreted as a temperature. The original file retains scale 1, offset 0 and no embedded unit; no Celsius conversion was written into it. Independent decoding found 3,410,133 non-NoData pixels and 9,549,867 NoData pixels, with raw valid range 11203–15805.

The actual monthly observation interval spans `2025-11-30T23:55:26.400000Z` to `2026-01-01T00:03:54Z`. The precise UTC interval is retained rather than rounded to a UTC calendar month. Cloud/observation gaps remain original NoData; embedded MODE overviews may be used for preview, while raw pixel inspection reads original pixels.

### JAXA license and original Collection conflict

Both original JAXA Items and Collections declare `proprietary`. [JAXA Research Data terms](https://earth.jaxa.jp/en/data/policy/) permit fee-free use with source credit and require advance notice for commercial use; gratis access is not a Creative Commons license. No commercial-use notification was sent as part of these tests.

The actual Collection documents use generic ID `collection`, whereas the Items name their full product identifiers. The JAXA-specific adapter validates only the corresponding fixed HTTPS URL, selected source proof and complete reviewed original Collection. It archives those original document bytes/hashes without making up an alias or attaching a falsely matching verified collection. `publisherCollectionDocumentAvailable:false` in provenance denotes that no matching publisher collection was attached; the separately archived raw Collection remains present in the document receipts. Other products retain strict ID validation. Original standalone Items have no `self` link; their exact retrieved URL is used for revalidation, and relative asset paths resolve directly on public HTTPS with no S3 transport exception.

## Preserved candidate and acceptance boundaries

The native catalog now contains 24 curated public products and 56 selectable layers. This count spans imagery, elevation, land cover, vegetation, hydrology, population, soil and climate; it is not a satellite/platform count or universal full-area scientific acceptance. Only the measured originals and declared date/layer/regional recipes are covered by these cases. Each original remains subject to the current transfer, decoding and resource limits. COG conformance and sensor/scientific calibration are not certified by the generic native preview.

[Separate soil research](research/next-channels-20261012-soil-jaxa.md) retains ISRIC SoilGrids as a candidate because its actual original uses unsupported custom IGH projection. Both full OpenLandMap SOC originals remain candidates because their own publisher checksums fail; neither mismatch was bypassed. Independent file readability does not promote those products to a released native connector.

The source directory's current 103 entries / 16 categories comprise 56 available, 6 requiring authorization and 41 planned entries. Protocols, local formats and multiple products on the same platform are counted separately. Browser Explore interactions for these four new products are still being exercised separately; this native report does not claim browser acceptance or an installed desktop upgrade.
