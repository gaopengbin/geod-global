# GEBCO 2026 official numerical region fixture

This complete 8,992-byte GeoTIFF was produced by the official [GEBCO regional download service](https://download.gebco.net/) on 2026-10-14, for longitude −30.1 to −30.0 and latitude 40.0 to 40.1. The request selected grid 1 (GEBCO 2026 surface elevation), data source 1 (bathymetry/elevation) and format 2 (numerical GeoTIFF). This is a **service-generated regional subset**, not the publisher's static global original, the TID grid, a rendered image or a new independent provider.

`reference.json` records the complete SHA-256 and all 576 original signed Int16 samples. Rasterio/GDAL and tifffile independently agreed on every sample. The grid is EPSG:4326, 24 × 24 cells at 15 arc-seconds, PixelIsArea, units metres relative to sea level, and embedded NoData −32767. Negative bathymetry is valid. The 2026 designation is a grid release, not a uniform measurement date or an accuracy guarantee. Source bytes and tags remain unchanged.

The GEBCO grid is public-domain data under its [terms and attribution requirements](https://www.gebco.net/data-products/gridded-bathymetry-data). Credit: GEBCO Compilation Group (2026), GEBCO 2026 Grid, Nippon Foundation–GEBCO Seabed 2030 Project, DOI 10.5285/4f68d5c7-45eb-f999-e063-7086abc036fa. Retain the official usage conditions; these data are not for navigation or safety at sea.

The private live service request, queue status, complete ZIP (including publisher documentation), both readers' metadata and byte receipts are in `.verification/remaining-backlog-20261014/maps/`. They are distinct from the repeatable offline decoder fixture. Offline regression does not imply that a new live download, desktop window or production deployment was tested.
