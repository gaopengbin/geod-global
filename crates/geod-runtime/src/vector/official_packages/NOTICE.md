# Reviewed official vector package index

The Microsoft index is derived, without row omission, from the publisher's complete
`2026-08-13/dataset-links.csv` (6,382,044 bytes, 30,340 rows), retrieved on 2026-10-14:

https://bfppub.blob.core.windows.net/%24web/2026-08-13/dataset-links.csv

Original CSV SHA-256:
`de61b569b0c23c364d61173632d8c5487ea30bf20912c55b9fa1ab4a89d01148`

`scripts/review-ms-building-index.py` verifies that entire original before generating
the finite index. Each tuple retains the publisher country, LOD9 quadkey, complete
part filename and rounded size. URLs are constructed only from these actual rows,
using the exact fixed release and publisher host. No missing tile is invented.

Microsoft Global ML Building Footprints data is licensed under CDLA Permissive 2.0:
https://cdla.dev/permissive-2-0/
https://github.com/microsoft/GlobalMLBuildingFootprints

Each downloaded package retains its original URL, index SHA-256, actual full-file
SHA-256, access date, release, source attribution and license URL. Original gzip
GeoJSONL is separate from its WGS84 GeoJSON map representation. Height/confidence
sentinels and original numbers are not replaced. A publication version is not an
acquisition date or a cadastral-quality guarantee.

The separate Geofabrik record identifies the whole original
`monaco-261011-free.shp.zip`, not a Microsoft partition:
https://download.geofabrik.de/europe/monaco.html
https://download.geofabrik.de/europe/monaco-261011-free.shp.zip

© OpenStreetMap contributors; processed by Geofabrik GmbH. The original archive
README declares ODbL 1.0 and an OSM data timestamp of `2026-10-11T20:21:06Z`:
https://opendatacommons.org/licenses/odbl/1-0/
https://download.geofabrik.de/osm-data-in-gis-formats-free.pdf

Retain the original README and all original bundle members, source notices and
ODbL database obligations when using or distributing derived data. The free GIS
bundle is the publisher's selected thematic schema, not raw OSM PBF/all tags.
