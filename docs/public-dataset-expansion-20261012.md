# Public dataset expansion acceptance — 2026-10-12

Five new representative numerical raster channels passed the actual native path: catalog and request review, metadata connection without a transfer, immutable source selection, saved project, explicit complete original download, independent full block decode, native preview and raw pixel inspection, and project/source reopening. These **five fresh native originals total 33,775,656 bytes**. The previous six-channel report is separate and remains unchanged.

The built-in product catalog now contains **11 products / 22 layer options**, selected through the original Explore dropdown alongside the 15 existing imagery providers. Separate [browser and automated verification](../prototype/qa/public-dataset-expansion-ui-verification.json) confirms month switching, a complete NASADEM download and raw-pixel inspection, shared collapsible review details, and a 768-pixel viewport. No actual model request or native-window click is claimed by this browser check.

The acceptance binary SHA-256 is `aae7ad3eaab3c87ac17a83600abb79edd5a41cf5d885d4246fa5895b6a4fdd1c`. Machine-readable receipts, exact source URLs, checksums, sampled coordinates and stored declarations are in [the JSON report](public-dataset-expansion-20261012.json). The reusable harness is [verify-public-dataset-connectors.py](../scripts/verify-public-dataset-connectors.py), with [five explicit cases](../scripts/public-dataset-expansion-cases.json). Originals, previews and logs are preserved privately in `.verification/public-datasets-expansion-native-20261012/`.

## Complete native originals

| Channel / tested original | Bytes | Native download observation | Grid / embedded file metadata | In-area raw sample |
| --- | ---: | ---: | --- | --- |
| CHIRPS v3 final monthly precipitation, 2024-01 | 22,243,892 | 113.891 s | 7,200 × 2,400, Float32, EPSG:4326, approximately 0.05°, Area; embedded NoData absent | 3.448835611343384 |
| NASADEM HGT V001, n00e006 | 1,528,533 | 2.875 s | 3,601 × 3,601, Int16, EPSG:4326, approximately 1″, Point; NoData −32768 | 583 |
| ALOS AW3D30 DSM, N000E006 | 1,963,609 | 3.734 s | 3,600 × 3,600, Int16, EPSG:4326, approximately 1″, Area; embedded NoData absent | 580 |
| SRTM GL1, n00e006 | 1,514,342 | 2.079 s | 3,601 × 3,601, Int16, EPSG:4326, approximately 1″, Point; NoData −32768 | 584 |
| HydroSHEDS v2r0 DIR, n00w050 | 6,525,280 | 1.265 s | 36,000 × 36,000, UInt8, EPSG:4326, 1″, Area; NoData 255 | 32 (northwest direction code) |

The download times come from explicit native queue submission until the complete file settled. They are single observations on the current machine/network, including queue and polling time, not speed guarantees. Separate Python research transfers took **24.578 s for CHIRPS** and **1.032 s for HydroSHEDS**; those external transfers confirmed file rules only and are not counted as native software-flow results or treated as a controlled performance comparison.

Rasterio independently decoded **1,352,174,402 raw samples across five bands**, including every original block. Auxiliary sentinel statistics do not add another set of samples to that total. Native grayscale PNGs parsed successfully, and **30 native pixel coordinates / 30 band values** matched the independent decoder and the actual embedded NoData flags. Each file also provided at least one finite, unmasked sample with its pixel center inside the requested area; one sampled point does not prove valid-data coverage of that area. All five original SHA-256 checksums, immutable source pins and saved project selections survived native inspection and reopening.

## Numerical interpretation and boundaries

- **CHIRPS:** The native 2024-01 TIFF is LZW-compressed Float32 with scale 1, offset 0 and no embedded NoData tag. Its grid spacing is actually `0.05000000074505806°`, producing tiny floating-point deviations from nominal global edges. The reviewed source declaration identifies −9999 as missing and precipitation as a monthly total in millimetres; these declarations are separate from the embedded file metadata. The original has 17,280,000 finite samples, including 12,439,253 sentinel values. Auxiliary acceptance calculations excluding −9999 give 4,840,747 samples (28.0136%), ranging 0–1433.258056640625. This is **not a processed software output**: generic native preview, file masks and raw pixel flags remain unchanged. A raw preview containing −9999 must not be presented as a scientifically masked precipitation map.
- **Elevation:** Metres and EGM96 vertical references come from publisher declarations; horizontal WGS84 GeoKeys do not prove the vertical datum. NASADEM/SRTM preserve Point registration and the half-pixel outer bounds; AW3D30 retains its Area grid. Their 583/580/584 samples demonstrate independent originals and are not accuracy comparisons. AW3D30 is a surface model, not a bare-earth terrain model. No vertical-datum conversion, mosaicking, clipping or resampling was accepted here.
- **HydroSHEDS:** The file contains categorical routing values, not observed flow or river vectors. The tested tile contains only 2,688,136 non-NoData samples (0.2074%); it is mostly ocean. Its in-area code 32 is one valid point, not a full-coverage claim. The source recipe is limited to the **100 DIR tiles actually listed** in the pinned publisher index and the fixed v2r0 release. ACC/ACA 64-bit products, other layers and continents are not implemented by this recipe. Some published DIR tiles exceed the native 512 MiB transfer limit; small-file success does not remove that gate.

All recipes download the complete published file selected by the area. This run does not establish arbitrary crop/processing support, all release months, all tiles, every source region or scientific accuracy. Existing ZIP packaging is for derived rasters; no original-source ZIP export or desktop-window acceptance is claimed.

## Metadata-only and request-boundary evidence

Fresh HEAD and bounded Range probes passed for CHIRPS 2024-01, 2026-02 and 2026-08, with file lengths 22,243,892 / 22,320,870 / 23,529,378 bytes. The live official monthly listing ended at 2026-08 in this run. Only 2024-01 received complete native original acceptance; other year/month options remain candidate references subject to actual release and possible 404. No preliminary monthly file was substituted.

HydroSHEDS n00w050 and n10w060 returned valid TIFF headers, while n30e110 returned 404. The [public index pin](research/hydrosheds-v2-index-20261012.json) records 100 DIR entries from the publisher's `download-index.json?v=20260830`, whose content SHA-256 is `28a3b61e2c6ade29fc4e8eed5c409a6704b73dad36e80324ca2d90934aecba6d`. The native whitelist matched that set exactly. Pure native requests rejected an unlisted Chinese tile, month 13, CHIRPS latitude above 60°N and year 1980, with no job created. These are request/metadata checks, not additional original downloads.

## Primary sources and licences

- [CHIRPS v3 publisher page](https://chc.ucsb.edu/data/chirps3), [version README](https://data.chc.ucsb.edu/products/CHIRPS/v3.0/README-CHIRPSv3.0.txt) and [monthly TIFF directory](https://data.chc.ucsb.edu/products/CHIRPS/v3.0/monthly/global/tifs/): final monthly land precipitation estimates, 1981 onward, 60°S–60°N. The official page combines a copyright-waiver/public-domain statement with CC BY 4.0; the source declaration retains attribution and CC BY 4.0 rather than inferring CC0.
- NASADEM, SRTM GL1 and AW3D30 use the exact public **OpenTopography static COG archive** URLs and publisher references recorded in each source snapshot. They do not call the OpenTopography compute API. Original publisher licence/attribution notices remain in those snapshots; the static mirror is not claimed to be the newest upstream edition.
- AW3D30 preserves the [JAXA research-data terms](https://earth.jaxa.jp/en/data/policy/), including source attribution and advance notification for commercial use. The tested OpenTopography archive is V3.2/3.1 from 2021, rather than a claim of the newest JAXA release.
- [HydroSHEDS v2 product](https://www.hydrosheds.org/products/hydrosheds-v2), [download page](https://www.hydrosheds.org/downloads-core-data-v2) and [technical documentation](https://data.hydrosheds.org/file/technical-documentation/HydroSHEDS_TechDoc_v2_0_0.pdf): CC BY 4.0 with attribution and the publisher's redistribution guidance; current released regions are North/South America.

## Researched but not accepted

[SoilGrids](https://docs.isric.org/globaldata/soilgrids/SoilGrids_faqs_02.html) is CC BY 4.0. Its current public pH capabilities expose 30 coverages and the fetched mean `DescribeCoverage` still declares native EPSG:152160 Homolosine. Native WCS supports other reviewed projections and has not accepted this grid; a reachable service or externally reprojected subset is not a completed software integration.

[TerraClimate v1.1](https://www.climatologylab.org/terraclimate.html) now describes monthly 1/24° products from 1950 onward using ERA5 anomalies. Its main distribution remains NetCDF/THREDDS, with older products migrated to FTP. Numerical NetCDF support is a separate requirement; no TerraClimate full original or native raster flow was accepted in this run.

The remaining directory has 41 planned entries. Subsequent batches should prioritize CBERS product adapters and numerical Esri/Impact Observatory land-cover files, then SoilGrids native projection support and safe archival ZIP handling for GHSL/CLCD/GLC_FCS products. NetCDF/GRIB/Zarr products require explicit format support. These remain planned; a reachable catalog, rendered ImageServer map, downloaded research sample or supported generic protocol is not a completed product integration.
