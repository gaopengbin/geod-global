# GeoD Global 0.1.0-rc.1 — Windows release candidate

Independent Windows 10/11 x64 geospatial desktop workspace with English and
Simplified Chinese UI. This prerelease is available for evaluation; the complete
product plan remains in development.

## Included in this candidate

- Search public Sentinel-2, Landsat, MODIS, Sentinel-1 RTC, NAIP and Copernicus
  DEM catalogues. The source selector distinguishes original-file availability
  from online previews, previews loaded after selection, and footprint-only
  catalogues.
- Preview MODIS NDVI/EVI, MODIS reflectance RGB, radar polarizations and public
  GLO-30/GLO-90 elevation on the exploration map. Nine public source entries have
  live catalogue and map-display checks in the documented test region. Preview
  images are distinct from completed original-file downloads.
- Create named multi-scene projects, add more scenes to an existing project,
  download supported assets, and inspect files and original pixel values in the
  local workspace. Compatible products support documented crop, mosaic,
  scientific RGB and quality-filter workflows, with verified delivery bundles.
- Use persistent thumbnail caching, compact file/task cards, resizable panels,
  a GeoD-branded desktop header, light/dark themes and system-tray background
  tasks. Closing the window keeps the application running; use the tray's
  **Quit** action to stop it before upgrading.
- Use the shared local CLI and MCP tools for supported jobs, projects, raster
  inspection and recipes. MCP mutations require explicit write enablement.
  An in-app conversational Agent is planned and is not included in this release.

The candidate also retains documented map-service, vector, offline-tile and
bounded 3D workflows. Each has its own support and acceptance limits; these do
not imply arbitrary GIS format or processing support.

## Fixes in this release

- Accept verified legacy NAIP catalogue identities and their matching 0.6 m
  four-band aerial COGs.
- Accept verified Sentinel-1 RTC platform and polarization identities, including
  supported Sentinel-1C/1D catalogue records.
- Retry temporary preview failures with bounded waits, cancel obsolete requests
  when switching sources, and retain a visible retry action on persistent errors.
- Keep scene-footprint fills transparent once imagery is displayed, preserving
  the preview's actual colours and grayscale values.

## Downloads

Choose the portable `.zip` or the per-user `-setup.exe` installer. Both contain the desktop application, CLI, examples, documentation, provenance and third-party notices. Verify downloads against `SHA256SUMS.txt`; `artifacts.json` records the exact source commit and artifact sizes and hashes.

Requires Windows 10/11 x64 and Microsoft Edge WebView2 Evergreen Runtime. The installer does not install WebView2. User data is retained on uninstall.

## Evaluation status

These binaries and installer are **unsigned**. This is a prerelease and is not
promoted to the latest stable release. The release workflow checks Linux/Windows
tests, production compilation, packaged CLI startup, archive contents, source
provenance and downloaded release asset hashes. Those checks do not establish
native GUI or clean-machine installation, upgrade and uninstall acceptance.

NASA Earthdata and Copernicus account setup is included. HLS, SRTM, VIIRS and
Copernicus SAFE protected original downloads remain disabled pending real-account
verification; their catalogues and footprints are available. No account is
required for the documented public-source workflows.

Processing is limited to supported products, coordinate systems and compatible
grids. General raster reprojection, arbitrary science formats, automatic updates
and the in-app Agent assistant are not included. Data use remains subject to each
provider's terms. Source downloads, previews and provider-original archives are
separate capabilities.

The source commit and build run are linked below. The README, provider status and
public-preview verification documentation at that commit describe the exact
tested scope. No cloud service, payment integration or commercial data entitlement
is implied.
