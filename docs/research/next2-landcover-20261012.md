# Cross-platform settlement and land-cover originals — 2026-10-12

JRC GHS-SMOD V2 reference epoch 2020 and UMD GLAD GLCLUC v2 each have a checksum-verified, fully decoded original. Both scoped connectors passed the parent's native download, preview, raw-value and persisted-project recovery checks and are enabled in the directory. These checks cover the selected originals, not every published object; browser/native-window visual acceptance is a separate boundary. CLCD and GLC_FCS30D are genuine public products but do not currently fit the direct-original delivery contract.

The accompanying [JSON evidence](next2-landcover-20261012.json) includes actual publisher records, complete S3 listing, HTTP headers, byte counts, checksums, physical TIFF tags, full raw-value counts and pixel coordinates. Originals and source-response archives remain under ignored `.verification/next2-landcover-20261012/`. No original was cropped, reprojected, resampled, relabelled or rewritten for testing.

## JRC GHS-SMOD — scoped implementation

The [JRC-managed AWS registry](https://registry.opendata.aws/ghs-smod/) publishes anonymous `jrc-ghsl/ghs-smod` access in `eu-central-1` and CC BY 4.0. The separately downloaded JRC `copyright.txt` also specifies CC BY 4.0. The actual complete S3 response contains 25 GeoTIFF objects: 12 WGS84 V2 reference epochs and 13 Mollweide objects, including one older V1 object. A public listing is availability evidence, not full acceptance of every object.

This connector selects only this independently checked original:

`https://jrc-ghsl.s3.eu-central-1.amazonaws.com/ghs-smod/r2023a/4326/30asec/2020/GHS_SMOD_E2020_GLOBE_R2023A_4326_30ss_V2_0.tif`

| Property | Actual original |
| --- | --- |
| Bytes | 20,566,541 |
| SHA-256 | `20661ac0f98166bd2253ce1155e131d589b0100fc2d5eed9dc7c632228071416` |
| S3 ETag | `b3e16bda987e3f40546ef829f457c8b1-3` — multipart, not whole-file MD5 |
| Grid | 43,202 × 21,384, one Int16 band, EPSG:4326, PixelIsArea |
| Compression | Deflate, 512 × 512 tiles; TIFF declares COG layout |
| Native NoData | Signed `-200`, representable in actual Int16 |
| Scale / offset | 1 / 0 |
| Full decode | All 923,831,568 original pixels, Rasterio 1.3.9 |
| Transfer | Complete HTTP 200 GET, approximately 177.7 seconds on the then-current local route |
| Palette | No embedded color table |
| Original bounds | `[-180.00791593130032, -89.10041610517152, 180.00874930942342, 89.0995831776456]` |

The [publisher machine metadata](https://human-settlement.emergency.copernicus.eu/data/ghs_smod2023.json) and its [product page](https://human-settlement.emergency.copernicus.eu/ghs_smod2023.php) identify Degree of Urbanisation stage I, detailed level 2, combining GHS-BUILT-S R2023 and GHS-POP R2023 / GPW v4.11. It is a modeled settlement typology. It is not population counts, a general land-cover map, a single satellite observation, individual building footprints or measured city boundaries. Reference epoch 2020 is retained without invented acquisition start/end timestamps. Other epochs, Mollweide originals and 2025/2030 products remain excluded.

The explicit class codes are 10 water, 11 very-low-density rural, 12 low-density rural, 13 rural cluster, 21 suburban/peri-urban, 22 semi-dense urban cluster, 23 dense urban cluster and 30 urban centre. All were present in the full decode. The original additionally has 10,479 code-0 pixels; the publisher legend does not explain 0. It is retained as an undocumented code, **not changed to NoData or assigned a class**. The publisher JSON says `int8` while the actual WGS84 TIFF is `Int16`; the physical file determines storage and raw-mask inspection. A grayscale preview does not reproduce the publisher categorical legend.

The two outer longitude columns have centres outside ±180 degrees, with actual water samples in those columns. The default general footprint guard rejects that geometry. The authorized scoped integration pins this exact source/original and fits the map to the physical WGS84 intersection `[-180, -89.10041610517152, 180, 89.0995831776456]`. It must preserve every original byte, column, value, NoData tag, transform and full original extent. This display fitting does not crop the scientific data, invent a publisher footprint or permit arbitrary overflowing rasters.

| Raw check | Row | Column | Original value |
| --- | ---: | ---: | ---: |
| Beijing, 116.4°E / 39.9°N | 5,903 | 35,568 | 30 |
| Ocean, 0° / 0° | 10,691 | 21,600 | 10 |
| Native NoData | 0 | 0 | -200 |
| Undocumented zero | 1,329 | 1 | 0 |
| Rural sample | 963 | 10,204 | 11 |
| West edge, centre -180.00374926465017° | 502 | 0 | 10 |
| East edge, centre 180.00458264277324° | 502 | 43,201 | 10 |

`crates/geod-runtime/src/datasets/ghsl.rs` defines `ghsl-smod`, one `settlement-class` layer and year option 2020. The request must retain a valid AOI intersecting the physical distribution. Month, country, region, season, other layer/version or epoch inputs are rejected. The source is an explicitly identified direct-file client recipe, never fabricated publisher STAC. Existing original-transfer and raster limits remain unchanged.

Dataset citation: Schiavina M., Melchiorri M., Pesaresi M. (2023), GHS-SMOD R2023A, European Commission / JRC, [doi:10.2905/A0DF7A6F-49DE-46EA-9BDE-563437A6E2BA](https://doi.org/10.2905/A0DF7A6F-49DE-46EA-9BDE-563437A6E2BA); identify the reviewed V2, WGS84 30ss, 2020 epoch and access date.

## UMD GLAD GLCLUC v2 — scoped implementation

The [author product page](https://glad.umd.edu/dataset/GLCLUC2020) links directly to an [anonymous Google Storage download page](https://storage.googleapis.com/earthenginepartners-hansen/GLCLU2000-2020/v2/download.html). That exact distribution page specifies CC BY 4.0, 10° tiles, 0.00025° unsigned numerical indices and a separate author [legend](https://storage.googleapis.com/earthenginepartners-hansen/GLCLU2000-2020/legend.xlsx). This combined land-cover/use product is distinct from the already connected Hansen forest-change product. Its annual maps are 2000, 2005, 2010, 2015 and 2020; the 2000–2020 change layer uses a different legend interpretation.

The complete 2020 URL index has 280 actual files. Positive original:

`https://storage.googleapis.com/earthenginepartners-hansen/GLCLU2000-2020/v2/2020/30N_030E.tif`

| Property | Actual original |
| --- | --- |
| Bytes | 44,495,039 |
| Publisher full-file MD5 | `6a5c47d302414cffb4c061cc5af0dd13` — independently matched |
| SHA-256 | `87406f90ee15916222ddceef02f8b7088e32e9b528b808f5a35aed40de68fcd7` |
| Publisher generation | `1694440708683567`, updated 2023-09-11 |
| Grid | 40,000 × 40,000 UInt8, EPSG:4326, PixelIsArea |
| Bounds / spacing | `[30, 20, 40, 30]`; 0.00025° |
| Compression | LZW, one 40,000-pixel row per strip |
| Native NoData | No scalar TIFF NoData declaration |
| Palette | Actual embedded 256-entry color table |
| Full decode | All 1,600,000,000 original pixels, approximately 12.2 seconds |

The initial complete-file HTTP 200 GET timed out after 44,040,192 bytes. A subsequent `If-Match` request for the same publisher generation supplied exactly the remaining 454,847 bytes. The assembled **whole original** then matched the independently fetched publisher object API full-file MD5 before all pixels were decoded. This is not partial Range or preview acceptance. Both original transfer receipts are retained.

Code 0 is a documented valid desert / 3% short-vegetation class; it is **not NoData**. Code 254 is ocean, 244 cropland and 250 built-up. The publisher legend uses 255 for missing data, while the actual TIFF has no scalar NoData tag. Any future connector must disclose that distinction, retain the original palette and indices, and never silently impose a file mask or infer continuous reflectance/temperature from combined class codes. The author also warns that map pixel counts alone are not unbiased statistically valid area estimators.

The scoped module `crates/geod-runtime/src/datasets/glad_landcover.rs` and `glad-landcover-reviewed-index.json` provide `glad-landcover-2020`, one `land-cover` layer and year option 2020. They select up to eight intersecting files from the actual complete 280-URL author index. Unlisted or excessive areas and other years/months/regions/countries/seasons/layers fail explicitly. The index's filename-derived extents are author-declared tile footprints and do not claim independently validated useful coverage for all 280 files. No fabricated observation timestamps or publisher STAC items are inserted. The selected 30N_030E original passed the parent native lifecycle checks, enabling this scoped directory entry.

Native acceptance reports are [GHSL](../next2-ghsl-native-acceptance-20261012.json) and [GLAD](../next2-glad-native-acceptance-20261012.json), both rechecked against binary `2658678889cfd816c67eb775d2f7a8b214f47da5e84e5eadcacbfe60c92ce2cd`. The accompanying research JSON records the exact report hashes and checked times; original scientific evidence above remains unchanged.

Citation: Potapov et al. (2022), [doi:10.3389/frsen.2022.856903](https://doi.org/10.3389/frsen.2022.856903).

## CLCD — actual cap blocker

The current author [Zenodo record 15853565](https://zenodo.org/records/15853565) and public API return 74 files with CC BY 4.0. Mainland bare GeoTIFFs are above the existing 512 MiB cap; for example the 2024 original is 794,945,897 bytes. Its provincial archive is 817,555,611 bytes and is ZIP, not a separately published provincial TIFF. No original raster was fully transferred or decoded. Filename/reference statements about Albers are not reported as physically verified TIFF CRS.

## GLC_FCS30D — actual container/transport blocker

The author [Zenodo record 15063683](https://zenodo.org/records/15063683) returns 38 files: 36 regional ZIP archives, a PDF guide and LYR legend, under CC BY 4.0. W175–W180 is a complete 178,031,961-byte archive, but its attempted full GET returned HTTP 504; no successful full-original checksum or raster decode is claimed. The [CASEarth product page](https://data.casearth.cn/thematic/glc_fcs30/314) separately documents login-free FTP. Neither delivery path is the current anonymous HTTPS bare-GeoTIFF workflow. The connector remains planned without archive-format expansion or fake standalone TIFF URLs.
