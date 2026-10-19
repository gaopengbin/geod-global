# Native Overture preview fixture

`overture-native.fixture.json` was captured from an actual native full-original
download and bounded AOI preview on 2026-10-14. It is a test fixture, not an
original Parquet file, substitute dataset or receipt of a live browser flow.

Source: Overture Maps Foundation, release 2026-09-23.1, base/bathymetry part 00000.
AOI: [-71.5, 40.5, -70.5, 41.5]. The eleven complete polygon features retain
original IDs, depth, nested sources/cartography, nulls, WKB hashes and coordinates.
Their source dataset is ETOPO/GLOBathy; depth represents metres below surface.

Base theme license: ODbL 1.0, with original source notices retained.
https://docs.overturemaps.org/attribution/
https://opendatacommons.org/licenses/odbl/1-0/

Whole-original SHA-256:
cfaf02a15f70713a52ed2f4367858d1b3d3be0dadba2a9aa4ee7faf0267ea23d

The fixture's `geoParquet` receipt retains its actual original URL, full-file
length/hash/rows and attribution. Preview length/hash identify the separate
GeoJSON preview. Source row-level null license fields remain null.
