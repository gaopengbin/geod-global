# JAXA JASMES MODIS monthly NDVI — 2026-10-13

The reviewed native integration is **September 2026, COG level 1**, for the real Terra+Aqua MODIS JASMES Version 811 monthly NDVI distribution. One complete original was independently fetched, fully decoded, and then downloaded through the native immutable-source lifecycle. Native inspection, five raw-pixel comparisons, and reopening after a kernel restart passed in a private test store. This does not claim browser/desktop visual acceptance, valid coverage at every selected location, or native acceptance of every global tile.

## Publisher and exact scope

The [official JAXA Earth root catalog](https://data.earth.jaxa.jp/stac/cog/v1/catalog.json) links the [actual monthly MODIS NDVI Collection](https://s3.ap-northeast-1.wasabisys.com/je-pds/cog/v1/JAXA.JASMES_Terra.MODIS-Aqua.MODIS_ndvi.v811_global_monthly/collection.json). Its latest child at review was September 2026. The monthly-normal, half-monthly, and SGLI products are distinct catalog entries and are not substituted. All eight actual level-1 items were independently retrieved and preserved unchanged in `jaxa-ndvi-reviewed-items.json`; the exact original Collection is retained in `jaxa-ndvi-reviewed-collections.json`.

The [JAXA JPMap scientific data description, page 5](https://www.eorc.jaxa.jp/jpmap/en/static/documents/data_source_en.pdf) describes the MODIS monthly NDVI as a maximum-value composite of daily daytime Terra/Aqua observations, on a 0.05° grid. Its older JPMap export NoData declaration is not used to override the current JAXA Earth distribution: the actual reviewed Item and complete TIFF both use **65535**. NDVI is dimensionless and is neither vegetation-cover percentage nor a monthly mean.

The resolver preserves the requested AOI and chooses intersecting 90° geographic tiles with half-open edges. It supports only `datasetId: jaxa-modis-ndvi`, `year: 2026`, `month: 9`, `layer: ndvi`. Other dates, layers, named regions, countries, or seasons fail explicitly. A finite reviewed month is not a claim that the historical catalog is fully integrated.

## Complete original and independent decode

[Exact publisher Item](https://s3.ap-northeast-1.wasabisys.com/je-pds/cog/v1/JAXA.JASMES_Terra.MODIS-Aqua.MODIS_ndvi.v811_global_monthly/2026-09/1/E090.00-E180.00/N00.00-N90.00.json) → [complete `ndvi` original](https://s3.ap-northeast-1.wasabisys.com/je-pds/cog/v1/JAXA.JASMES_Terra.MODIS-Aqua.MODIS_ndvi.v811_global_monthly/2026-09/1/E090.00-E180.00/E090.00-N00.00-E180.00-N90.00-ndvi.tiff).

- Bytes: **2,589,470**. First independent full transfer: **0.996 seconds** on this machine and route; this is an observed probe, not a speed guarantee.
- SHA-256: `0b1810637f3a61e32aa2ee27794ec7bafd396131afb855eb09d9cd964dedc1cf`.
- Actual MD5: `c4e679479ea89ec77dc8b61e620f96d8`, matching the observed simple HTTP ETag. The publisher Item declares no `file:checksum`; neither the ETag nor local SHA-256 is described as publisher checksum verification.
- Grid: EPSG:4326, bounds `[90,0,180,90]`, **1800 × 1800**, UInt16, one band, 0.05° pixels, 256 × 256 tiles. All **3,240,000** original-grid pixels were decoded with Rasterio.
- Valid pixels in the published raw range: **1,211,123**. Scalar NoData pixels: **2,028,877**. Unknown raw values: **0**. Observed valid DN: **2–9954**.
- Actual publisher valid DN: **2–9966**, physical NDVI = DN × **0.0001**, offset **0**. Original TIFF embedded scale/offset remain **1/0**, with no embedded physical unit. Original samples were not rewritten.
- Precise acquisition interval: **2026-09-01T00:00:00.000000Z** through **2026-09-30T23:59:59.999999Z**, with `datetime: null`. October processing/creation is not an October observation.
- Embedded overviews use MODE; the native preview is a raw first-band grayscale stretch. It is not a calibrated vegetation-color map and does not change the original-grid raw samples.

Five independent raw samples include a Beijing-area value **6032 → 0.6032 NDVI**, raw minimum **2**, raw maximum **9954**, and two distinct scalar NoData positions **65535**. The complete report preserves each row/column and coordinate; each native pixel response matched both the raw integer and separate NoData flag.

## Native lifecycle acceptance

The native smoke used a newly compiled, copied `geod-runtime` executable and a new private test store under `.verification/jaxa-ndvi-20261013/native-store-2`. It did not use the user's desktop store, Earth Engine project, account, API key, or model connection.

1. Resolve and connect the exact reviewed product/area; persist immutable publisher Item and Collection document receipts.
2. Pin `{snapshotId, assetKey: ndvi}`, save a project, and submit one complete original transfer through the existing STAC project download path.
3. Verify successful terminal status, byte count, and the same independent-original SHA-256.
4. Inspect the native 1800 × 1800 EPSG:4326 original and independently decode its generated **768 × 768 PNG** preview.
5. Compare the five raw-pixel responses and their NoData flags with the independent full-grid decode.
6. Stop and restart only the private test kernel; reopen the same project, snapshot, successful job, inspection, preview, and raw samples. The job count remains **one**; reopening initiates no new transfer.

The first smoke interrupted at a test assertion for the Windows `\\?\` path prefix. The second interrupted at the test's expected sample-type capitalization (`uint16` versus actual native `UInt16`). Both original downloads succeeded and matched the independent hash. The corrected test resumed the second private store, completed inspection and reopening, and did not redownload. These were acceptance-script corrections, not bypasses of native validation or provider errors.

## Metadata conflict and license

The original Collection ID is the generic string `collection`, while Items name the full monthly MODIS NDVI product identifier. The scoped adapter checks the exact fixed endpoint, original document, selected immutable product proof and source. It archives the unchanged conflict while retaining `provenance.collection: null`; it adds no ID alias and no assertion of a matching verified collection. The shared strict checks still apply to other sources. Items contain no self link; revalidation uses the independently retrieved exact Item URL. Relative assets already resolve on public HTTPS, so no S3 transport adapter is used.

Both publisher Item and Collection declare `proprietary`. The [JAXA Research Data terms](https://earth.jaxa.jp/en/data/policy/) allow fee-free use/distribution with source attribution, including attribution to other contributing agencies. The module credits **JAXA/NASA**. Commercial use requires prior notification to JAXA under the linked policy; no such notification was sent by this local test. Public free delivery is not a Creative Commons declaration.

## Files and checks

- Native module: `crates/geod-runtime/src/datasets/jaxa_ndvi.rs`; three module tests passed after shared dispatch integration.
- Independent evidence: `.verification/jaxa-ndvi-20261013/{metadata-receipts,transfer-receipt,independent-decode}.json`, complete unchanged original TIFF.
- Native evidence: the same root's `native-{connect,snapshot,project,job,inspection,pixels,reopen,acceptance}.json`, `native-preview.png`, and private stores.
- `native-acceptance.json` records `nativeAccepted: true` for this exact complete east-Asia original and lifecycle. Frontend/native-window visual review and other products are separate acceptance work.
