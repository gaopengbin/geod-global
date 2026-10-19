# Complete original GeoParquet and bounded map preview

The original `overture-maps` source has one finite native profile:
Overture `2026-09-23.1`, `base/bathymetry`, official part `00000`.
Buildings, places, transport, other parts and automatically selected new releases
are outside this profile.

The complete published Parquet file is downloaded without a server-side subset,
stored independently in the managed `parquet-originals` directory and verified
against its full length and SHA-256. It contains 407,277 rows and 16 row groups;
the original is 52,018,294 bytes. Original geometry WKB and all nested attributes
remain in that unchanged file.

Original SHA-256:
`cfaf02a15f70713a52ed2f4367858d1b3d3be0dadba2a9aa4ee7faf0267ea23d`

Official original index:
https://stac.overturemaps.org/2026-09-23.1/base/bathymetry/00000/00000.json

Official data:
https://overturemaps-us-west-2.s3.us-west-2.amazonaws.com/release/2026-09-23.1/theme=base/type=bathymetry/part-00000-304001d2-9e92-5aeb-8db5-60be5bd09d96-c000.zstd.parquet

## Independent limits

| Representation | Bound |
| --- | --- |
| Original GeoParquet container | 512 MiB, streamed to a temporary file |
| Footer | 4 MiB |
| Original schema | 128 physical columns, 64 KiB schema text |
| Original row groups | 128 groups maximum; 100,000 rows per group |
| Each row group's declared uncompressed data | 32 MiB; same bound for each compressed/uncompressed column chunk |
| GeoJSON preview | Existing 20 MiB, 50,000 features, 500,000 coordinates |
| Original attribute page | 1–20 rows and at most 24 KiB including envelope |

These are new explicit limits for this GeoParquet profile. Existing GeoJSON,
gzip GeoJSONL, Shapefile, GeoPackage and OSM limits are unchanged. The fixed
full-file hash is checked before Parquet metadata or data pages are decoded.
The reviewed original's largest uncompressed row group is 9,942,868 bytes.
Only one row group is read at a time, using Apache Parquet's record reader.

## Scientific representation

The original declares GeoParquet 1.1.0, WKB, Polygon/MultiPolygon and a publisher
bbox covering column. Its omitted CRS field means OGC:CRS84 as specified by
GeoParquet; explicit null or an unknown CRS is refused. WKB is always XY
longitude/latitude. The existing bounded raw-WKB reader is reused without
clipping, simplifying, rounding, repairing geometry or transforming vertical data.

Selection uses each publisher row's bbox intersection with the current AOI.
It can include geometries that extend outside that AOI, and bbox intersection
does not establish exact geometry intersection. Matched complete geometries and
all original non-geometry columns become a separate managed GeoJSON preview.
Every feature records its original row index and WKB SHA-256. The full original
is still retained when the selected output is refused by the existing preview
budget; no partial preview is registered.

`depth` is depth below surface in metres, describing the source's underwater
polygons. These ETOPO/GLOBathy-derived thematic representations are not a new
seabed measurement, pointwise raster depth or bathymetric accuracy guarantee.
Original GERS IDs, `depth`, feature version, cartography hints, nested `sources`,
nulls and source update-time strings remain unchanged.

The Base theme is licensed under ODbL 1.0. Preserve the publisher's attribution
and source-specific notices. Original row-level null license fields stay null;
they are not rewritten to an invented license. See:

- https://docs.overturemaps.org/schema/reference/base/bathymetry/
- https://docs.overturemaps.org/attribution/
- https://geoparquet.org/releases/v1.1.0/
- https://arrow.apache.org/rust/parquet/file/reader/trait.RowGroupReader.html

## Saved state and export

`VectorAsset.format = geoparquet-preview` describes the bounded GeoJSON file.
Its `bytes`, `sourceSha256`, `geojsonSha256`, feature count and coordinate count
describe that preview. The `geoParquet` receipt separately identifies the whole
original's URL, bytes, SHA-256, row count, group count, schema, geo metadata and
the AOI selection. Original content and schema are checked again on inspect.

Export GeoJSON writes the selected preview. Export original streams the complete
Parquet file into a destination temporary file and checks its full bytes/hash
before replacing the destination. The legacy in-memory vector-original method
refuses this format so a preview cannot accidentally be exported as the original.
The dedicated browser original endpoint also streams the complete container.

The unified Explore component exposes complete-original download, actual map
preview, reopen of a saved preview for the same AOI, independently labelled
original/preview hashes, original attribute pagination and complete original
export. Native acceptance and browser acceptance are separate evidence.

Native reference acceptance used AOI `[-71.5,40.5,-70.5,41.5]`: 11 complete
features, 554 coordinates, 22,907 preview bytes. Every selected coordinate,
nested property and original WKB hash matches independent PyArrow/Shapely reads.
Save/reopen and complete original export pass; a changed original is rejected.

Evidence: `.verification/original-backlog-20261014/vector-research/`
(`overture-reference.json`, `overture-native-download-reopen.json`,
`overture-native-runtime`). The frontend's test fixture is a captured native
preview used for tests only; it is not a published original or proof of a live
browser download.
