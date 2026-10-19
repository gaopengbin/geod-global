# Soil channels and two JAXA Earth candidates — 2026-10-12

This review downloaded five complete publisher GeoTIFFs and independently decoded every pixel with Rasterio. Two JAXA monthly products have a scoped native module prepared; **native download, preview, raw-value inspection and reopen acceptance are separate and pending in this report**. The three soil originals remain research candidates, for the concrete reasons below. No Earth Engine shared project, account, credential, server-side export, client reprojection or artificial STAC item was used.

## Result boundaries

| Product and exact distribution | Complete original | Independent result | Current decision |
| --- | ---: | --- | --- |
| ISRIC SoilGrids 2.0 pH mean, 0–5 cm, publisher 1 km aggregation | 78,793,346 bytes | Official SHA-256 matches; 577,632,308 pixels decoded; custom Interrupted Goode Homolosine without an EPSG code | Planned: current native and map projection support cannot correctly display this original |
| OpenLandMap current annual SOC, 2018, 1 km, 0–30 cm declaration | 96,693,810 bytes | All 774,316,800 pixels decoded; published MD5 multihash conflicts with complete bytes | Planned: checksum conflict and insufficient numeric log encoding evidence |
| OpenLandMap older static annual SOC, 2018, 1 km, `b30cm` point-depth asset | 97,194,247 bytes | All 774,316,800 pixels decoded; different published MD5 also conflicts | Planned: retain separate release and depth semantics; do not bypass checksum |
| JAXA AMSR2 v3 nighttime monthly soil moisture, December 2025, COG level 1, E90–180 / N0–90 | 140,042 bytes | EPSG:4326; 810,000 pixels decoded; valid DN, scalar NoData and seven published error flags distinguished | Native module ready; runtime acceptance pending |
| JAXA SGLI v3 daytime monthly land-surface temperature, December 2025, COG level 1, E90–180 / N0–90 | 3,679,954 bytes | EPSG:4326; 12,960,000 pixels decoded; exact DN-to-Kelvin declaration retained | Native module ready; runtime acceptance pending |

Transfer timings below describe this machine and route during the probe, not a service-speed guarantee. Complete-file equality was tested against each publisher's declared checksum when present. A local hash and a matching HTTP ETag are recorded separately when the publisher declares no file checksum.

## SoilGrids: correct original, incompatible projection

The [official WebDAV instructions](https://docs.isric.org/globaldata/soilgrids/WebDav.html) and the [aggregated-product README](https://files.isric.org/soilgrids/latest/data_aggregated/README.md) lead to a directly downloadable [0–5 cm pH mean, 1 km GeoTIFF](https://files.isric.org/soilgrids/latest/data_aggregated/1000m/phh2o/phh2o_0-5cm_mean_1000.tif). This distribution is the publisher's aggregation of 250 m predictions, rather than the original 250 m prediction grid. The scientific factor from [the units table](https://docs.isric.org/globaldata/soilgrids/SoilGrids_faqs_01.html) is stored pH divided by 10. The original TIFF retains scale 1, offset 0 and no embedded unit; it was not numerically converted.

- Size: 78,793,346 bytes; full transfer 274.969 seconds.
- SHA-256: `d49a413ffe3d483ed41359855fd548509decfb5930368dcee7cfc3eff55d2d97`, matching the actual directory's `checksum.sha256.txt`.
- Grid: 39,812 × 14,509; Int16; 512 × 512 tiles; 1,000 m spacing; custom `Interrupted_Goode_Homolosine`; no EPSG identifier.
- NoData: -32768. Full decode found 128,227,186 valid pixels and 449,405,122 NoData pixels; valid raw 34–103, corresponding to pH 3.4–10.3.
- License: CC BY 4.0, as declared by the publisher's aggregation README.

The original is independently readable. It is not accepted as an in-product source because the current software cannot faithfully handle this custom projection. Supplying a false EPSG code or a newly reprojected file as the publisher original would change the evidence. Four named-place probe positions happened to be NoData; those samples do not establish local usable coverage. The full valid-value histogram is retained in the private report.

## OpenLandMap: two real deliveries, two checksum conflicts

The [official STAC guide](https://docs.openlandmap.org/020-stac.html) documents a static Wasabi catalog. The current `stac.openlandmap.org/catalog.json` path returned a browser application; its live configuration points to [the current OpenGeoHub catalog](https://stac.opengeohub.org/v1/cat/openlandmap). Both actual JSON catalog routes were inspected. Neither an HTML page nor a synthetic converted record was treated as STAC evidence.

The [current 2018 annual SOC item](https://stac.opengeohub.org/v1/cat/openlandmap/collections/log.oc_iso.10694_go/items/log.oc_iso.10694_go_20180101_20181231) declares a [96,693,810-byte Ceph asset](https://ceph.opengeohub.org/arco/soildb/usdapc_oclog_m_1km_bg0..30cm_20180101_20181231_go01_epsg4326_v1.0.0_srl03.tif). The full response and size agree, but its MD5 multihash fails:

- Declared multihash: `d50110c604b64dfd03217ffa8c1c4820a863c1`; declared MD5 payload `c604b64dfd03217ffa8c1c4820a863c1`.
- Actual MD5: `d73bcb6420c9e250e0d776becc680e58`.
- Actual SHA-256: `55e03a1da36c9dd14cd29f903b06e5c46a4e79d5f4a0aae7ac3f34a333e4a061`.
- The complete bytes remain quarantined under an `.part` filename; no successful download receipt was fabricated.

The [older static item](https://s3.eu-central-1.wasabisys.com/stac/openlandmap/log.oc_iso.10694/log.oc_iso.10694_20180101_20181231/log.oc_iso.10694_20180101_20181231.json) declares a distinct [`b30cm` asset](https://s3.openlandmap.org/arco/log.oc_iso.10694_m_1km_b30cm_20180101_20181231_go_espg.4326_v20230608.tif). It also downloaded completely in 40.125 seconds and fails its own checksum:

- Declared MD5: `ac36f4dd974c5112d3edc2ba59261641`.
- Actual MD5: `9824e9e719fc2cb9999f09fe174cb1a4`.
- Actual SHA-256: `99fa9cb2b0e782d65ad9edb51562f54d6ebfd1184cc459f1b7db3da671dc1790`.

Both files have 43,200 × 17,924 pixels, EPSG:4326, UInt8, NoData 255, 210,028,773 valid pixels and 564,288,027 NoData pixels. The observed histograms and selected samples match, but this review does not assert byte equality or whole-array identity between the releases. The publisher declares logarithmic g/kg values; this review did not find a sufficiently specific inverse encoding for these older 1 km products. It does not invent `exp(raw/10)`, interpret raw bytes directly as g/kg, or replace concentration with carbon density. The older `b30cm` point depth is not silently relabeled as the newer 0–30 cm interval.

Other actual pH originals were checked for feasibility: the older 250 m asset is 1,785,702,233 bytes and 172,800 × 71,698 pixels, exceeding both the transfer and grid limits; newer 120 m and 30 m originals are still larger. Their source JSON is retained unchanged, including malformed coordinate ordering or date declarations where observed. These candidates remain planned.

## JAXA: actual publisher monthly COG distributions

The [JAXA Earth API](https://data.earth.jaxa.jp/en/) publishes converted numerical COGs with original STAC metadata. The [actual root catalog](https://data.earth.jaxa.jp/stac/cog/v1/catalog.json) had 112 child links on inspection. This review adds two distinct products; it does not count the already integrated ALOS World 3D again. It also does not describe JAXA's converted monthly COGs as original mission HDF delivery.

The implemented scope is exactly December 2025, COG pyramid level 1, one nighttime soil-moisture measurement and one daytime LST measurement. Sixteen actual publisher items were retrieved: four longitude sectors and two latitude sectors per product. Every item is a 90° tile, has one data asset and EPSG:4326, and matches the checked-in original JSON snapshot. A valid selected AOI chooses only its intersecting tiles using half-open edges; unsupported dates, modes, layers, regions and invalid AOIs fail explicitly. No demonstration AOI or alternate month is substituted.

### AMSR2 soil moisture

[Exact publisher item](https://s3.ap-northeast-1.wasabisys.com/je-pds/cog/v1/JAXA.G-Portal_GCOM-W.AMSR2_standard.L3-SMC.nighttime.v3_global_monthly/2025-12/1/E090.00-E180.00/N00.00-N90.00.json) → [complete `SMC` asset](https://s3.ap-northeast-1.wasabisys.com/je-pds/cog/v1/JAXA.G-Portal_GCOM-W.AMSR2_standard.L3-SMC.nighttime.v3_global_monthly/2025-12/1/E090.00-E180.00/E090.00-N00.00-E180.00-N90.00-SMC.tiff).

- Dataset ID `jaxa-amsr2-soil-moisture`; request layer `smc`; original asset key `SMC`.
- Full transfer: 140,042 bytes in 1.781 seconds; SHA-256 `fff9b57eafe2f0d312226bcab9c71c1e435fa8fc732b8f1dabb413f7cb0be3f1`.
- Grid: 900 × 900, 0.1° WGS84 spacing, 256 × 256 tiles, Int16. Source resolution and satellite footprint are not inferred from grid spacing.
- Publisher valid DN 0–600: physical volumetric percent = DN × 0.1; original TIFF scale remains 1 and offset 0. No fixed depth interval is declared by these items.
- Scalar NoData -32768: 581,670 pixels. Valid DN: 221,130 pixels, observed range 11–600. Published additional error -32767: 7,200 pixels; none were converted into moisture or newly masked NoData. All seven published error codes -32767 through -32761 remain declared; the tested original only contains -32767 among those seven.
- Beijing sample column 264 / row 500: DN 31, interpretation 3.1%; error sample column 0 / row 4: DN -32767, scalar NoData false, scientific value unavailable. The error flag and NoData flag are separate.
- Actual acquisition interval: `2025-12-01T00:00:00.885000Z` to `2025-12-31T23:56:41.333000Z`.

### SGLI land-surface temperature

[Exact publisher item](https://s3.ap-northeast-1.wasabisys.com/je-pds/cog/v1/JAXA.G-Portal_GCOM-C.SGLI_standard.L3-LST.daytime.v3_global_monthly/2025-12/1/E090.00-E180.00/N00.00-N90.00.json) → [complete `LST` asset](https://s3.ap-northeast-1.wasabisys.com/je-pds/cog/v1/JAXA.G-Portal_GCOM-C.SGLI_standard.L3-LST.daytime.v3_global_monthly/2025-12/1/E090.00-E180.00/E090.00-N00.00-E180.00-N90.00-LST.tiff).

- Dataset ID `jaxa-sgli-lst`; request layer `lst`; original asset key `LST`.
- Full transfer: 3,679,954 bytes in 1.093 seconds; SHA-256 `8172496fa42359cc8ba0f38ab79a2eb586bfaec9085a9d0839f54f2bd41093a5`.
- Grid: 3,600 × 3,600, 0.025° WGS84 spacing, 256 × 256 tiles, UInt16. This distributed grid does not imply independent 0.025° sensor detail; the publisher's source monthly map is approximately 5 km.
- Publisher valid DN 11203–16860: Kelvin = DN × `0.019999999552965164`, offset zero. The original stores scale 1, offset 0 and no embedded unit. No Celsius conversion was written into the file.
- Scalar NoData/error 65535: 9,549,867 pixels. Valid DN: 3,410,133 pixels, observed range 11203–15805. No unknown raw values were found.
- Beijing column 1056 / row 2000: DN 13811, interpreted 276.219993826 K. This is daytime surface temperature, not two-metre air temperature.
- Actual acquisition interval: `2025-11-30T23:55:26.400000Z` to `2026-01-01T00:03:54Z`; the December bin spans the UTC calendar boundaries. The original precise timestamps are retained.

### License, collection conflict and immutable receipts

Both actual items and their collections declare `proprietary`. [JAXA Research Data terms](https://earth.jaxa.jp/en/data/policy/) permit fee-free use with source credit and require advance notification for commercial use. Free public download does not create a Creative Commons license. No commercial-use notification was sent during this research.

The two [SMC](https://s3.ap-northeast-1.wasabisys.com/je-pds/cog/v1/JAXA.G-Portal_GCOM-W.AMSR2_standard.L3-SMC.nighttime.v3_global_monthly/collection.json) / [LST](https://s3.ap-northeast-1.wasabisys.com/je-pds/cog/v1/JAXA.G-Portal_GCOM-C.SGLI_standard.L3-LST.daytime.v3_global_monthly/collection.json) Collection documents use the generic ID `collection`, whereas each Item names its full product identifier. This is a real publisher metadata conflict. The JAXA-only `validate_collection` adapter checks the exact selected source proof, corresponding fixed HTTPS URL and full original Collection document. It allows archiving unchanged bytes and a hash receipt, **without synthesizing an ID alias or treating it as a matching verified collection**. Other products retain strict collection-ID handling. Actual runtime acceptance of this exception is pending.

The items have no `self` link. The module returns the exact independently retrieved item URL for revalidation. Original relative TIFF hrefs already resolve on public HTTPS and require no S3 transport exception. All original item values, asset roles, dates, scientific units, DN ranges, error lists, license and URLs are compared with the finite snapshot. Changes fail rather than silently altering source provenance. Exact Collection comparison also rejects upstream catalog changes until reviewed.

Neither item declares `file:checksum`; local SHA-256 and matching simple HTTP ETag MD5 are recorded as observed transfer evidence, not publisher file-checksum verification. The original HTTP response uses `binary/octet-stream`, so native content validation must retain its TIFF signature check and reject JSON/HTML masquerading as imagery.

## Artifacts and next acceptance

- Native module: `crates/geod-runtime/src/datasets/jaxa.rs`; no Cargo or service restart was run by this reviewer.
- Exact publisher snapshots: `jaxa-reviewed-items.json` (16) and `jaxa-reviewed-collections.json` (2), alongside the module. Each was compared with independently retrieved original document bytes and hash receipts.
- Private evidence root: `.verification/next-channels-20261012-soil/`; separate raw bytes, complete originals, HTTP receipts, full pixel histograms and independent reports are retained there.
- `jaxa-final-review.json` contains the two exact `{datasetId,bounds:[116,39,117,40],year:2025,month:12,layer:smc|lst}` cases, complete-file hashes and five distinct original raw samples per product.
- Runtime acceptance must still compare fresh native-downloaded original hashes, content metadata and previews, five raw samples including the SMC non-NoData error, persisted original documents and reopened project behavior. A research preview is not native-preview acceptance.

The structured companion report records `nativeAccepted:false` until actual runtime evidence is added separately.
