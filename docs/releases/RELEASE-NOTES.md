# GeoD Global — Windows evaluation

Independent Windows x64 desktop application with English and Simplified Chinese UI.

- Search Sentinel-2 scenes through Earth Search and download supported source assets.
- Inspect local SCL GeoTIFF metadata and pixels, compare layers on their native UTM grid, and crop rectangular areas without resampling.
- Save and rerun recipes, export verified delivery bundles, and use the local CLI or MCP server.

## Downloads

Choose the portable `.zip` or the per-user `-setup.exe` installer. Both contain the desktop application, CLI, examples, documentation, provenance and third-party notices. Verify downloads against `SHA256SUMS.txt`; `artifacts.json` records the exact source commit and artifact sizes and hashes.

Requires Windows 10/11 x64 and Microsoft Edge WebView2 Evergreen Runtime. The installer does not install WebView2. User data is retained on uninstall.

## Evaluation status

These binaries and installer are **unsigned**. This is a prerelease and is not promoted to the latest stable release. Automated checks cover Linux/Windows tests, production compilation, packaged CLI startup, archive contents, source provenance and downloaded release asset hashes. They do not establish native GUI acceptance, clean-machine installation, upgrade or uninstall acceptance. General raster reprojection, multiband scientific processing and automatic updates are not implemented.

The source commit and workflow run are linked by GitHub. Feature details and current limitations are documented in the README at that tag. No cloud account or payment integration is implied.
