# Vegetation and ocean channel acceptance — 2026-10-13

Two bounded JAXA Earth products are now connected to the existing Explore workflow. They use immutable publisher Items, the standard project/original-download path, native raw inspection and local reopening. They do not enable the whole JAXA catalog or original mission-resolution/HDF products.

| Product | Reviewed scope | Actual northeast original |
| --- | --- | --- |
| JAXA PALSAR-2 forest / non-forest | FNF v2.1.0, 2020, JAXA Earth COG level 1; eight unchanged 90° tile Items | 809,136 bytes; 3600 × 3600 UInt8; EPSG:4326; 0.025°; NoData 0 |
| JAXA SGLI monthly ocean chlorophyll | CHLA v3 daytime/monthly, September 2026, COG level 1; eight unchanged 90° tile Items | 9,421,740 bytes; 3600 × 3600 UInt16; EPSG:4326; 0.025°; NoData 65535 |

Only the northeast original from each product was fully transferred and independently decoded, then accepted through the native lifecycle. The eight-Item indexes support geographic selection; the other complete originals are not described as individually download-tested.

## Source and scientific integrity

- [JAXA Earth publisher catalog](https://data.earth.jaxa.jp/stac/cog/v1/catalog.json), [FNF publisher documentation](https://www.eorc.jaxa.jp/ALOS/en/dataset/fnf_e.htm), [JAXA data policy](https://earth.jaxa.jp/en/data/policy/).
- FNF retains class 0 No Data, class 1 Forest (publisher label `x > 90%`), class 2 Forest (`10% < x < 90%`), class 3 Non forest and class 4 Water. These are discrete class codes. The 0.025° distributed grid is approximately 2.8 km at the equator, not the source mission's 25 m delivery. The annual interval is not a single acquisition.
- The FNF online first-band grayscale preview now stretches the exact reviewed publisher range 1–4 instead of the UInt8 storage range 0–255. This fixes the nearly black display. Original bytes, codes and NoData remain unchanged; this is not a continuous canopy-percentage conversion or a thematic color legend.
- CHLA preserves valid DN 0–65534, including valid zero, and separate scalar NoData/error code 65535. Physical concentration is `DN × 0.0015999999595806003 mg/m³`, offset zero, from unchanged publisher metadata. The original TIFF embeds scale 1/offset 0. The raw preview is grayscale; it does not perform calibration or a logarithmic ocean-color display.
- Original acquisition intervals are retained per tile. The northeast CHLA Item spans 2026-09-01T00:27:24.000000Z through 2026-09-30T23:15:29.000000Z; October creation is not October observation.
- CHLA's generic Collection ID conflict is preserved and bounded to the exact reviewed Collection. FNF Item and Collection full identities match and remain strict. No aliases, private-bucket access or guessed HTTPS mirrors are added.
- Both products retain proprietary JAXA research-data terms and source credits. The linked policy describes advance notice for commercial use. No notice was sent as part of these local tests. Local SHA receipts do not imply publisher checksum verification.

## Verified lifecycle and UI

1. Actual metadata discovery keeps the requested AOI, exact reviewed date/layer and original asset key (`FNF` or `CHLA`). Unsupported substitutions fail explicitly.
2. Save one immutable asset selection in a new private project; transfer the complete original. Size and SHA-256 match the independently fetched original.
3. Native inspection returns the real typed grid, a decodable 768 × 768 PNG and matching raw/NoData results for five FNF and six CHLA samples.
4. Restart the private kernel; reopen the same project, snapshot, job, inspection and pixels without another transfer.
5. Browser Explore real flow verifies source selection, automatic metadata retrieval, preview, saved selection, original download, My Data file display and raw-pixel reads. Source switches keep the AOI and clear prior selections/previews.
6. Desktop UI verifies category entries, automatic retrieval, original project/download and local result display. The new Ocean category separates marine concentration products from inland hydrology and optical imagery.

Original SHA-256 values:

- FNF: `34e8f8ba3ab1a0b65e91d94089f7fe6a2e738b2e0d36cd06198584f01a8bee9f`
- CHLA: `5c0cec5a090d1a8f8624901e09f0aa601e55a8f0248abac9b69bea843b13834a`

Evidence is kept locally under `.verification/channels-next3-20261013/`, `.verification/jaxa-fnf-20261013/` and `.verification/new-channel-probe-20261013/`. It includes independent complete decodes, immutable native receipts, browser full-output hashes, native reopening and UI captures. These local evidence directories are not release assets.

Runtime library tests: 749 passed, 16 opt-in live tests ignored. Relevant UI suites and the actual OpenLayers GLSL stretch test passed; client validators reject changed dates, NoData and geographic/provenance fields. Repository checks, frontend build and debug desktop build passed. No deployment, Git publication or public release was performed.

Directory inventory is 109 entries: 63 ready connection/product paths, 6 catalog/auth paths and 40 planned entries. These counts are directory entries, not 69 individually accepted original-download providers.

## Next candidate: GLC_FCS30D

An actual CASEarth explicit-FTPS original was downloaded: 21,832,902 bytes, 18,554 × 18,554, 23 UInt8 year planes for 2000–2022. All 7,917,771,068 samples were independently decoded block by block; observed codes match the publisher legend. Native/application acceptance remains false.

The current native path has no FTPS transfer adapter and accepts at most 16 TIFF samples per pixel. This original has 23. Its spatial grid fits the current tiled-grid limit; the older SCL reader's 64M limit does not apply. The publisher's fill codes 0 and 250 are separate from the absence of an embedded TIFF NoData tag. It needs bounded FTPS transport, explicit time-plane selection and real preview/raw/reopen acceptance before the directory can be enabled. No exact-original HTTPS mirror was verified. Evidence: `.verification/glcfcs-probe-20261013/`.
