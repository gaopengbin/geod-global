# Offline AOI reference map

- Dataset: Natural Earth `ne_50m_land.geojson`, 1:50m land polygons.
- Project: https://github.com/nvkelso/natural-earth-vector
- Exact source: https://raw.githubusercontent.com/nvkelso/natural-earth-vector/ca96624a56bd078437bca8184e78163e5039ad19/geojson/ne_50m_land.geojson
- Commit: `ca96624a56bd078437bca8184e78163e5039ad19`.
- Local SHA-256: `e874b27a51d146452be360cafb3cc50c86001074a67d534113e6534682f9826b`.
- Terms: Natural Earth data are public domain; see https://www.naturalearthdata.com/about/.

The bundled map is a coarse geographic reference for drawing WGS 84 search
bounds. It is not satellite imagery, a precise coastline or a processing input.
Searches use the exact validated coordinates, not geometry inferred from this
display layer.

## Administrative reference layers

Both layers below use the same pinned upstream commit and Natural Earth public
domain terms as the land file:

| Bundled file | Upstream GeoJSON | SHA-256 | Coverage |
| --- | --- | --- | --- |
| `natural-earth-50m-admin-0-countries.geojson` | `geojson/ne_50m_admin_0_countries.geojson` | `3e458fc036ad0a66411f2c1e6cac49c5d7bfb81cb1123bc513b22511a2b7fdeb` | 242 country/map-unit features worldwide |
| `natural-earth-50m-admin-1-states-provinces.geojson` | `geojson/ne_50m_admin_1_states_provinces.geojson` | `69a0e06e640b2d505858ae1cb63034e4677f3000b35a98e16312932b98c426b9` | 294 state/province features in Australia, Brazil, Canada, China, India, Indonesia, Russia, South Africa and the United States |

Upstream URLs are `https://raw.githubusercontent.com/nvkelso/natural-earth-vector/ca96624a56bd078437bca8184e78163e5039ad19/` followed by the paths above. Natural Earth uses a de facto cartographic boundary viewpoint by default. These generalized boundaries are for navigation, not authoritative legal geography. Selecting an area only creates a WGS 84 bounding rectangle for the current Earth Search catalog; it does not submit or clip to the administrative polygon.
