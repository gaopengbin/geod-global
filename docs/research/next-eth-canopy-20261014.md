# ETH Global Canopy Height 2020 — next-batch source research

This is one new scientific product: dense model-predicted canopy top height, trained using GEDI RH98 supervision and Sentinel-2 imagery. It is separate from the already connected GEDI L3 mean RH100 observations and is not a new year/band counted as another source. The separately distributed STD prediction-uncertainty files are not connected by this adapter. Implementation/native/UI acceptance have distinct receipts; this research alone does not establish the software workflow.

## Primary publication and access

- [Author product page, methods and CC BY 4.0 declaration](https://langnico.github.io/globalcanopyheight/).
- [Author tile browser](https://langnico.github.io/globalcanopyheight/assets/tile_index.html), retrieved as an unchanged 2,608,985-byte HTML file. SHA-256: `e140cda0253ab46cd268c27ed6122d39d116dbb0be0010419f3e9d65b4e01903`.
- [Dataset DOI](https://doi.org/10.3929/ethz-b-000609802). The article/data citations and DOI are retained from the author page; a failed browser fetch of the redirect is not represented as an independent catalog receipt.
- [Nature article](https://doi.org/10.1038/s41559-023-02206-6).

The mechanically extracted, otherwise unchanged publisher FeatureCollection contains exactly **2,651** CHM tile records and their original geographic geometry/bbox, identity, complete delivery href and STD popup link. Its separately serialized JSON SHA-256 is `695e54e193a8407fc183b719b7492075cf2c632c23512092bdab5dd0a44c21bd`; this JSON is a reviewed client fixture, not publisher STAC. The source file and embedded published index remain under the private verification directory. `next-eth-canopy-reviewed-index.json` contains that full FeatureCollection unchanged as JSON values. No missing filename, fallback region, generated AOI export or new STAC Item was manufactured.

## Checked original

[Actual N51E003 CHM original](https://libdrive.ethz.ch/index.php/s/cO8or7iOe5dT2Rt/download?path=%2F3deg_cogs&files=ETH_GlobalCanopyHeight_10m_2020_N51E003_Map.tif) was obtained with anonymous public HTTPS. HTTP range returned the requested initial 256 KiB with total length, and the later independent full GET retrieved all **171,132,359 bytes** in **219.625 seconds**. Complete original SHA-256: `2d1247ba652d858dd1152c418d5ccd9bcdf5f8d34ec307b0275899ded550e7ac`. This is a local full-file digest; no publisher checksum declaration was fabricated.

| Original declaration | Independently checked value |
| --- | --- |
| Raster | 36,000 × 36,000, 1 UInt8 band, LZW, contiguous samples, 1,024 × 1,024 tiles |
| Horizontal CRS | EPSG:4326, WGS84, PixelIsArea |
| Original outer bounds | 3°E, 51°N, 6°E, 54°N |
| Pixel spacing | 0.00008333333333333333° = 0.3 arc-second |
| Stored units | Height in metres, scale 1, offset 0 |
| NoData | Original TIFF declaration 255 |
| Embedded overview factors | 2, 4, 8, 16, 32, 64 |
| Complete independent decoding | All 1,296,000,000 pixels decoded in bounded block iteration, 7.453 seconds |
| Valid original pixels | 475,070,444, range 0–50 m, including 48,511,541 valid zeros |
| Original NoData pixels | 820,929,556 |

The 768-pixel independent PNG was decoded from the complete original and visually inspected: the North Sea is absent/transparent and the Dutch/Belgian land surface has varying model-height texture. The tile centre itself is ocean NoData; use **[5.1, 51.6, 5.3, 51.8]** for the UI acceptance AOI, which intersects the original interior Dutch land. Six reference raw samples and flags are retained in `land-climate/eth-full.json`; no research TIFF will be injected into a software job store.

Four actual published header probes succeeded: N51E003 171,132,359 B; N39E114 301,900,420 B; N21E111 304,474,031 B; and N45E006 407,025,206 B. Each is a 36,000² geographic UInt8/LZW original with NoData 255. These header checks establish encoding and file-size feasibility, not full-file or scientific acceptance of all four originals or all 2,651 listed files.

## Scientific and implementation boundaries

The author calls the product nominal 10 m ground sampling distance. Its actual geographic delivery is 0.3 arc-second, so ground-cell dimensions vary with latitude. It is not a 12,000-pixel UTM Sentinel-2 source tile; no reprojection or downsampling converts it into one. Integer-metre storage is not one-metre measurement accuracy. Predicted RH98-supervised canopy top height is not raw laser measurement, ground elevation, RH100, forest/nonforest classification, biomass or an observation from the current year. Zero remains valid, and 255 remains NoData. A grayscale image is visualization, not a scientific quality mask; separately published STD uncertainty remains outside this connector.

`next_eth_canopy.rs` selects the exact recorded CHM href/footprint, fixed 2020 and height layer only, with at most eight intersecting records. Invalid or unsupported dates, months, country exports, other layers, absent extents and oversized selections fail explicitly. The generic direct-raster connector retains ordinary header validation, immutable source provenance, full-file jobs and raw-pixel inspection. Existing 512 MiB transfer, 131,072-pixel tiled-axis, 8-billion-pixel grid and decoder budgets are retained. Listed files that exceed those budgets do not become downloadable because they appear in the catalog.

CC BY 4.0 is the actual author-product license, with author, data and article acknowledgement retained. This is ordinary user-selected public original access, with no Google Earth Engine execution, credentials, paid infrastructure, notification or commercial-procurement step. The legacy WorldClim/FABDEM research retained in the same private directory is replacement research only and is not counted as additional connected products.

## Acceptance receipts

Implementation module tests and software-native/UI/reopen receipts will be appended after the coordinating agent registers the module, freezes the runtime and starts a fresh download through the actual Explore interface. The independent research file above establishes the publisher original and reference samples only.
