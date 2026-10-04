# Third-party assets and data

- **GeoD application mark:** existing blue/cyan GeoD A1 G symbol, included with
  its original MIT notice and source/hash record under `prototype/public/brand`.
  The same mark is resampled into the Windows executable, tray and installer
  icons; it is not a dependency on the domestic product checkout.
- **Desktop caption controls:** `tauri-plugin-decoration` 3.0.5, MIT. The locked
  crate and its full license are included in the resolved Rust inventory.
- **Offline 3D renderer:** CesiumJS 1.146.0, Apache-2.0, with its pinned npm
  graph. Workers, supporting assets, credits and decoders are bundled locally;
  the unmodified aggregate `LICENSE.md`, `ThirdParty.json` and
  `ThirdParty.extra.json` are included in frontend resources and the dependency
  notice inventory. Cesium's built-in on-screen credit remains visible. No ion
  token or hosted asset service is configured. The optional public acquisition
  example is generated Cesium sample data under its
  [CC0 generator notice](https://github.com/CesiumGS/3d-tiles-samples-generator/blob/main/README.md),
  fetched only after an explicit user action. Acceptance also uses Khronos'
  [BoxTextured sample](https://github.com/KhronosGroup/glTF-Sample-Assets/blob/main/Models/BoxTextured/README.md)
  with CC-BY-4.0 and the separate Cesium logo/mark notice; that asset is QA data
  in isolated storage and is not bundled application content. User-supplied 3D
  datasets retain their own permission and attribution declarations.
- **SAFE JP2 decoding:** `jpeg2k` 0.10.1 (MIT / Apache-2.0) with `openjp2`
  0.6.1 (BSD-2-Clause, Rust port of OpenJPEG). The resolved crates preserve
  upstream notices. `roxmltree` 0.21.1 is MIT OR Apache-2.0. These dependencies
  are native runtime libraries; GDAL is used only for independent tests.
- **GeoPackage horizontal coordinates:** `proj-core` and `proj-wkt` 0.11.0
  (MIT OR Apache-2.0), pinned native runtime libraries from
  [proj-rust](https://github.com/roteiro-gis/proj-rust). Resolved upstream source
  and notices remain in the Rust inventory. Each layer records its actual datum
  operation; regional per-coordinate method selection and external grids are
  outside the current implementation. The small OGR-generated GeoPackage fixture
  is synthetic QA data, not a bundled third-party production dataset; GDAL is a
  QA dependency and is not required by the application.
- **Shapefile topology and text:** `geo` 0.33.1 (MIT OR Apache-2.0), from
  [georust/geo](https://github.com/georust/geo), and `encoding_rs` 0.8.35
  ((Apache-2.0 OR MIT) AND BSD-3-Clause), from
  [hsivonen/encoding_rs](https://github.com/hsivonen/encoding_rs), are pinned
  native runtime dependencies. Resolved source and applicable notices remain
  in the Rust inventory. Shapefile coordinate conversion also uses the pinned
  proj-rust libraries above. The independent PyShp/GDAL fixture is own synthetic
  QA data; those producers are not application dependencies. Natural Earth
  Lakes and the deliberately rejected Land sample are public-domain QA inputs
  kept outside the package; their exact provenance and limits are recorded in
  [Shapefile acceptance](../shapefile-local.md).

- **Inter typeface:** SIL Open Font License 1.1; full notice is included in
  `fonts/LICENSE-Inter.txt`. Embedded font files originate from the repository's
  `prototype/public/fonts` directory.
- **SCL display legend:** class meanings and display colors follow the
  [Sentinel Hub SCL legend](https://custom-scripts.sentinel-hub.com/custom-scripts/sentinel-2/scene-classification/)
  by Sentinel Hub, published under CC BY-SA 4.0. The application independently
  implements the class/color mapping. The license is available at
  [Creative Commons](https://creativecommons.org/licenses/by-sa/4.0/legalcode.en).
- **Sample scene thumbnails and catalog records:** Copernicus Sentinel-2 data,
  exposed by Earth Search / Element 84. Per-file URLs, hashes and attribution
  are retained in the embedded sample manifest. Data use follows the
  [Copernicus Sentinel legal notice](https://sentinels.copernicus.eu/documents/247904/690755/Sentinel_Data_Legal_Notice).
- **Offline area-selection reference map:** Natural Earth 1:50m land, worldwide
  country and selected state/province polygons, public domain. The bundled
  GeoJSON files are pinned to the upstream commit and SHA-256 recorded in
  `prototype/public/basemaps/SOURCE.md`. These are geographic orientation layers,
  not analytical data, legal boundaries or an online tile service.
- **Live downloaded raster data:** not shipped inside this package. Provider
  attribution, original asset URL and source checksum accompany each local job
  and generated crop sidecar.
- **Copernicus DEM GLO-30 Public:** public 2021 DSM COG distribution, provided
  by DLR / Airbus, the European Union and ESA, hosted on AWS. Original height
  rasters are downloaded only when requested and are not shipped in the app.
  Access and use follow the dataset's
  [published licence](https://registry.opendata.aws/copernicus-dem/). The
  retained `cop-dem-response.json` is a captured Earth Search catalogue record;
  no original elevation tile is included in the bundled samples.
- **USDA NAIP aerial imagery:** the retained `naip-response.json` is a captured
  Planetary Computer catalogue response. No original aerial raster is bundled.
  The [official collection](https://planetarycomputer.microsoft.com/api/stac/v1/collections/naip)
  links its Public Domain terms to USDA FSA policy, although its `license` field
  currently says `proprietary`; preserve the source and its published terms.
  Attribution identifies USDA NAIP, Esri processing and Microsoft distribution.
  This adapter downloads the reviewed four-band Planetary Computer COG, not the
  separate AWS Requester Pays raw-source distribution.
- **NAIP Deflate decoding:** `flate2` 1.1.10 (MIT OR Apache-2.0) is a locked
  native runtime dependency for bounded, lossless four-channel tile decoding.
- **VIIRS HDF5 reading:** `hdf5-reader` and `hdf5-core` 0.9.1
  (MIT OR Apache-2.0) are locked read-only Rust dependencies. Their published
  source is maintained in [netcdf-rust](https://github.com/roteiro-gis/netcdf-rust).
  Default optional features are disabled; no system HDF5 library is required.
  The bundled VIIRS HDF5 fixtures are explicitly synthetic QA artifacts generated
  with h5py/NumPy, not NASA observations or evidence of original-product access.
- **Native local vector file dialogs:** `rfd` 0.17.2, MIT, a locked Windows-only
  desktop dependency maintained at [PolyMeilex/rfd](https://github.com/PolyMeilex/rfd).
  It provides user-operated open/save dialogs owned by the main window; the
  loopback browser adapter does not receive native file paths.
- **OSM polygon classification:** the unmodified `osm-polygon-features` 0.9.2
  table by Martin Raifer, CC0-1.0, is embedded in the native converter. Its
  license, pinned source record and original JSON are included under
  `THIRD-PARTY/vendored/osm-polygon-features`; checksums are verified when notices
  are collected. No upstream JavaScript is executed.
- **User-opened or explicitly queried OSM extracts:** not bundled application data. When an Overpass
  JSON file is opened, original OSM identity, tags, dataset timestamp and
  `© OpenStreetMap contributors` attribution are retained with the
  [ODbL notice](https://www.openstreetmap.org/copyright). The application does
  not configure a public Overpass instance as its default backend. Online queries
  require a user-provided service endpoint. Acceptance reports record explicitly
  requested small QA extracts; their full files are kept
  in isolated local verification storage.

`inventory.json` enumerates the locked Rust graph and installed npm packages,
including build dependencies and platform-optional packages. This is deliberately
broader than a claim that each package is linked into the binaries. Exact MPL
crate source is included under `source/`; the remaining source is identified by
package version and repository in the inventory. License files missing from a
published crate are fetched from its declared official GitHub repository at the
exact commit recorded in that crate's `.cargo_vcs_info.json`. Where proj4rs or
selectors also omit a standalone file in that commit, the package includes the
unmodified standard Apache-2.0 or MPL-2.0 terms, published license declaration and
complete original crate source with its copyright headers. Lerc similarly includes
its original JavaScript copyright/license header and Apache-2.0 terms. Platform
binding packages inherit the texts from their matching same-version parent package;
these origins are distinguished in the inventory.

- **OSM XML / PBF acceptance controls:** upstream test files from `b-r-u/osmpbf` at `4bc5ce41eedbbeb263c2fb7091dc48d28134df56` are retained as synthetic parser controls, with [source receipts and MIT notice](../../crates/geod-runtime/fixtures/osm/SOURCE.json). The separately authored controls use Pyosmium 4.3.1 only in isolated developer verification, not in the application. The [Seatac derived subset](../../crates/geod-runtime/fixtures/osm/SEATAC-SOURCE.json) retains OSM geographic data under ODbL, with parent SHA-256 and the exact retained way IDs; it is not an unchanged provider original. These files are test fixtures, not bundled user map data. The application reader is original repository code based on the [OSM PBF schema](https://wiki.openstreetmap.org/wiki/PBF_Format), using the already-pinned roxmltree and flate2 dependencies. [Acceptance and limitations](../osm-local-data.md).
