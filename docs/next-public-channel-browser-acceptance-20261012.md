# Four additional public products — actual browser acceptance

The Chinese local Explore workflow was exercised through the classified source picker, using the repository's frozen native runtime `b63daf248f7c042d7109594d7df944b61fb6e576342da1f09b21026dc2c92580` on loopback port 4318. This is local browser and native API acceptance, not an installed desktop update or public deployment.

| Product | Actual selection | Original bytes | Browser checks |
| --- | --- | ---: | --- |
| USGS 3DEP | Current 1 arc-second `n41w074`, published 2026-09-17 | 20,582,281 | Real TNM record; publication distinct from observation; EPSG:4269 map placement and map-click pixel lookup; valid negative and zero heights; original -999999 NoData |
| CHELSA | `cmi_mean` V2.1, fixed 1981–2010 normal | 244,959,851 | Entire original; EPSG:4326 map placement; actual expanded outer grid retained; original negative -323 and NoData 32767 |
| JAXA AMSR2 | Nighttime monthly soil moisture, December 2025, E90–180/N0–90 | 140,042 | Actual original interval; DN 31; -32767 retained as an unmasked error code; -32768 scalar NoData |
| JAXA SGLI | Daytime monthly surface temperature, December 2025, E90–180/N0–90 | 3,679,954 | Actual UTC interval 2025-11-30–2026-01-01; raw DN 13811; NoData 65535; no Celsius conversion |

Each flow required resolve, publisher metadata review, explicit original selection, project save and a separate download action. Four distinct projects were created in the preview store. The products were chosen by the user-facing picker; no acceptance script created their projects or initiated their transfers. All four actual transfer receipts record `fresh` with zero resumed bytes.

The original files total **269,362,128 bytes**. Independent Rasterio/GDAL read every base-resolution block, totaling **928,832,544 pixels**, and verified file size and SHA-256 before and after reading. Eleven distinct coordinates observed in the browser matched the actual originals; 35 coordinates were checked in the separate backend audit. Raw inspection, scalar NoData, thumbnails and saved source pins all matched. An error code's lack of scalar NoData is not a claim of scientific validity.

The final runtime was restarted after all tasks settled. All 16 task records and 14 project records were unchanged on readback. Each of the four new projects was reopened through the browser, its georeferenced preview displayed, and a previously checked original value read again. Older records were checked for loading compatibility, not all independently decoded again.

Changing from CHELSA to the two JAXA products retained the explicit Beijing search bounds. No substitute sample region was introduced. The two JAXA products share the publisher's `N00.00-N90.00` item name, but their collection, asset, snapshot, original URL and project pins remained separate. The slightly expanded CHELSA outer cell edges were preserved in file metadata; only the display fit extent intersects the physical globe. Unsupported coordinate systems and larger overflows remain rejected.

The directory now contains 103 entries: 56 connected, 6 catalog-only/authentication-dependent, and 41 planned, across 16 categories. The numerical product API offers 24 products and 56 layers. These totals include products, services and formats; they are not a count of satellite platforms. JAXA support in this batch is explicitly limited to the verified December 2025 distributions and preserves its research-data terms, including advance commercial-use notification.

Validation: 688 native tests passed (15 ignored), 395 frontend unit tests and 471 UI integration tests passed. Clippy, production build, repository isolation, contracts and recipe checks passed. The existing file-size, pixel-count and decoding guards were retained. No model request or Earth Engine compute call was used.

Machine receipt: [browser acceptance](../prototype/qa/next-public-channel-ui-verification-20261012.json). The separate [native acceptance](next-public-channel-native-acceptance-20261012.md) distinguishes initial private-store transfers from final-runtime reinspection. Screenshots and the full independent browser-file audit remain in the ignored local verification directory.
