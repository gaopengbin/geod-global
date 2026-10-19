# Five public product families — native complete-original acceptance

2026-10-12. **Passed: 18 complete originals, 5 product families.** The isolated native runtime performed real metadata discovery, explicit selection, project saving, complete original download, raw raster inspection, native PNG preview, original pixel reads, and reopening after a runtime restart. No source metadata or completed file was injected from the preceding research downloads.

The runtime was frozen with SHA-256 `8d77e502ce74928cf0df0d1c6704794cffd47bc80180b7f75126479abe67b542`. The fresh acceptance store used loopback port 4666; the runtime was stopped after verification, and that port was confirmed unused. The user preview on 4318 was not operated by this acceptance run. No accounts, model calls, or Earth Engine compute requests were used.

## Actual evidence

| Product / reference release | Complete files | Original bytes | Independently decoded original pixels |
| --- | ---: | ---: | ---: |
| DE Africa Fractional Cover, 2025, `x218y075` | 11 | 63,880,707 | 112,640,000 |
| DE Africa WOfS annual summary, 2025, `x218y075` | 3 | 6,734,211 | 30,720,000 |
| WSF settlement distribution, 2015 v2 and 2019 v1, `36_-2` | 2 | 37,662,623 | 1,011,420,288 |
| GMW mangroves, 2020 v3, `S05E039` | 1 | 123,267 | 20,250,000 |
| CGLS land-cover family, 2019 permanent-water fraction | 1 | 36,681,970 | 7,969,076,457 |
| **Total** | **18** | **145,082,778** | **9,144,106,745** |

- All 18 original file SHA-256 values matched the native transfer receipts before and after inspection. The 14 FC/WOfS file SHA-1 values also matched their separately fetched publisher checksum sidecars.
- Rasterio 1.3.9 / GDAL 3.6.4 decoded each entire original grid block by block. No thumbnail, overview, cropped window, or generated export was substituted for a complete original.
- **128 native pixel coordinates** matched the independently decoded values and original embedded NoData declarations, including positive class values, valid zero values, scalar 255, and Float32 NaN.
- All **18 native PNG previews** parsed successfully. After a private runtime restart, all 18 saved projects, original source snapshots, jobs, representative pixels, and original native preview hashes matched.
- **23 invalid-input cases, 46 resolve/connect checks** rejected unsupported areas, years, months, layers, excessive tile selections, and caller-provided URL/credential fields. They created no file jobs. Semantic failures returned 400; invalid schema fields returned 422.
- Archived publisher document bytes, hashes, properties, geometry, asset declarations, and collection availability were preserved. The product recipe and permitted transport receipts were checked separately from publisher metadata.

The complete machine-readable receipt is [new-public-channel-native-acceptance-20261012.json](new-public-channel-native-acceptance-20261012.json); the repeatable selections are [new-public-channel-cases-20261012.json](../scripts/new-public-channel-cases-20261012.json). Private complete originals, native preview PNGs, and actual API response captures remain under the ignored `.verification/new-public-channels-native-20261012/` directory. The observed sum of transfer durations was 209.282 seconds for these specific files on this connection; it is **not a general download-speed guarantee**.

## Scientific and resource boundaries

**Fractional Cover:** the nine component-percentile files are independent annual model summaries for green vegetation, non-green vegetation, and bare soil. They are not a simultaneous composition to add to 100%. The tested percentiles have an embedded scalar NoData value of 255. In contrast, the actual `count_valid` TIFF is Int16 with valid counts 0–41 and **no embedded NoData tag**; the actual `qa` TIFF is UInt8 with valid codes 0–2 and **no embedded NoData tag**. Any auxiliary documentation or preset sentinel declaration is distinct from the original TIFF mask: native inspection retained these actual missing-tag/valid-zero differences and did not insert 255 into either original. A QA code is not a percentage or confidence estimate. [Publisher specification](https://docs.digitalearthafrica.org/en/latest/data_specs/Fractional_Cover_specs.html).

**WOfS:** all 9,384,383 usable frequency samples were compared against the fresh native `count_wet / count_clear` originals; maximum absolute error was `2.8610229518832853e-8`. All 855,617 zero-clear-count samples retained NaN frequency. A valid zero frequency and a missing estimate are different states; frequency is a proportion of clear observations classified wet, not a fraction of physical time under water, present flood extent, or water depth. [Publisher specification](https://docs.digitalearthafrica.org/en/latest/data_specs/Landsat_WOfS_specs.html).

**WSF / GMW:** WSF's actual settlement value is **255**, while the tested WSF TIFFs have no embedded NoData tag; zero background remains a readable value with `noData=false`. The tested GMW v3 file has mangrove value **1** and embedded **NoData=0**. GMW zero therefore cannot independently establish valid non-mangrove ground coverage. Different releases and edge tiles retain their actual original grids; older GMW continental grids are outside the current native caps. [Original source research and grid differences](research/new-channels-20261012-wsf-gmw.md).

**CGLS:** only the reviewed permanent-water fraction layer was accepted here; other CGLS layers and the other reference years were not validated by this one original. The original is percent cover, with valid zero, positive 100, and embedded NoData=255. Its 2019-nrt/v3.0.1 TIFF tags state an input period of **2017-04-01–2020-03-31**, separate from the publisher's 2019 calendar release interval. The preserved original S3 asset points to the erroneous `gls/` path; only this exact reviewed asset receives the recorded public `cgls_landcover/` transport mapping. The other 13 original asset declarations remain unsupported. Its 7,969,076,457-pixel tiled grid remains just below the unchanged 8-billion-pixel cap. [Source/path and original-tag research](research/new-channels-20261012-cgls-isda.md).

Existing resource limits remain in force: complete files ≤512 MiB; tiled axes ≤131,072 and grid ≤8 billion pixels; stripped axes ≤65,536; bounded chunks, intermediates, preview budgets, and deadlines. Selecting an area does not crop or reproject an original. One tested tile/file per release does not establish exhaustive continental downloads, every available year, all requested-area valid pixels, or local scientific accuracy. Missing metadata and out-of-scope selections fail rather than substituting an example area or alternate source.

This report establishes the **native** workflow. Production frontend validation and actual browser interaction/visual acceptance are separate checks; this report does not itself claim a published installer, deployment, or completed browser QA.
