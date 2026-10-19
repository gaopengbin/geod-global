# IO annual land-cover original-file research — 2026-10-12

The publisher's public STAC API and complete numeric classification download were verified. This report records the old reader's actual dimension failure; it does not claim that a curated preset or larger-image inspection was already accepted. The subsequent curated acceptance report must use its own frozen executable and store.

## Distribution and provenance

- The official [AWS registry](https://registry.opendata.aws/io-lulc/) describes 2017–2024 annual 10 m, nine-class maps. The [publisher's AWS tutorial](https://docs.impactobservatory.com/tutorials/aws-open-data-exchange/aws-open-data-exchange.html) provides the public endpoint `https://api.impactobservatory.com/stac-aws/` and collection `io-10m-annual-lulc`.
- The complete unsigned bucket listing returned 5,902 objects in six pages. It contained 757 original TIFFs for 2024, and no `_2025.tif` or `_2026.tif` objects. This is the observed public release boundary, not an inference from a marketing page.
- The raw STAC collection declares `license: proprietary`, while its description, registry and [Maps for Good licence](https://docs.impactobservatory.com/lulc-maps/maps-for-good.html) identify CC BY 4.0. Preserve the original collection and explicitly explain the reviewed licence discrepancy. The annual open-data licence does not apply to separately ordered IO Monitor products.
- Current Planetary Computer collection `io-lulc-annual-v02` states 2017–2023 and uses the same large supercell grids. It is not a finer tiling solution. Its PNG rendering endpoints were not treated as original classifications.

## Complete representative original

The published [29W 2024 classification TIFF](https://io-10m-annual-lulc.s3.us-west-2.amazonaws.com/29W_2024.tif) was fetched as a complete 12,593,521-byte file. The independent download took 13.641 seconds; the separate native queued-to-settled observation took 15.531 seconds. These timings are different measurements and must not be merged.

The original is 29,338 × 89,448 pixels, one UInt8 band, LZW-compressed, with 512 × 512 tiles, a TIFF palette, 10 m spacing, EPSG:32629, PixelIsArea and embedded NoData 0. Its SHA-256 is `ed2f9bebdd374a1f725821db3981c4b8b4e0ec07a2200bbfc0cfba588d297ac9`.

Rasterio independently decoded all 2,624,225,424 original pixels. There were 15,379,549 unmasked pixels (0.5861%) with class codes 1, 2, 4, 8, 9, 10 and 11; most of this maritime supercell is masked. This proves actual numeric data, not classification accuracy or full requested-area coverage. The publisher's current legend is 0 NoData; 1 water; 2 trees; 4 flooded vegetation; 5 crops; 7 built area; 8 bare ground; 9 snow/ice; 10 clouds; 11 rangeland. Old codes 3 and 6 must not be silently substituted.

The old frozen runtime `aae7ad3eaab3c87ac17a83600abb79edd5a41cf5d885d4246fa5895b6a4fdd1c` successfully performed real API connection, AOI/time search, immutable source selection, saved project creation, complete download, SHA verification and project reopening. Discovery started no download. Inspection returned HTTP 400: `Generic raster dimensions exceed 65,536 pixels per side`. The private test runtime was stopped; no user's desktop process was operated.

## Reader boundary review

A bounded expansion should keep stripped images at 65,536 per axis and permit only tiled images up to 131,072 per axis, with an explicit total grid cap. An 8-billion-pixel cap admits ordinary low-latitude IO grids around 5.9 billion pixels and this 2.62-billion-pixel representative. The 134,279-pixel-high Svalbard supercell still exceeds the proposed axis limit and must remain a truthful unsupported case.

The current reader decodes selected chunks rather than allocating a full image. Preserve its 128 MiB worst-case per-chunk allocation, 32 MiB intermediate buffer, 1 MiB per IFD value, 512 MiB cumulative preview budget, 768-pixel preview edge and 30-second preview deadline. `chunk_layout` already checks complete interleaved count using checked multiplication; pixel and preview indices are bounded by that count. Preview source coordinate multiplication uses u64. The locked `tiff` 0.11.3 implementation removes tile padding from chunk data dimensions, bounding those dimensions by image extents; unchecked u64 multiplication was not established as an actual overflow under these image limits. Defensive checked sample arithmetic remains appropriate.

The original palette and raw class codes should remain unchanged. A grayscale preview is a display limitation; a future thematic palette must be described as display styling rather than a rewritten numerical classification. Sensor calibration, crop processing, original ZIP packaging and scientific coverage are outside this proof.

Machine-readable current receipts are in [io-lulc-candidate-20261012.json](io-lulc-candidate-20261012.json). Raw publisher documents, complete originals and isolated native records remain under the ignored `.verification/io-lulc-candidate-20261012/` directory. They are a new research run, separate from the previous eleven-source acceptance runs.
