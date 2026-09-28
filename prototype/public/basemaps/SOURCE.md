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
