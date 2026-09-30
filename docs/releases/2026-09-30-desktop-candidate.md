# Windows desktop candidate — 2026-09-30

Distribution target: the product website and GitHub downloads. Microsoft Store,
accounts, subscriptions and cloud collaboration are outside this candidate.
This is release preparation, not authorization to publish the build.

## Changes and current acceptance

- The desktop is the primary development entry point. Its own Rust manager
  performs local work through 26 scoped IPC commands; no companion HTTP service
  is required by the desktop.
- Repeated application launches restore the existing window before trying to
  open the exclusive store. The official single-instance plugin is pinned to
  2.4.5, compatible with the existing pinned Tauri 2.11.6 runtime.
- Projects and files have separate library views, real local-file thumbnails,
  shared blue controls and compact metadata. Prior browser checks covered light,
  dark and narrow layouts. Native-window workflow acceptance remains separate.
- The new release CLI processed two previously downloaded original Sentinel-2
  true-color COGs, `S2C_50TML_20260928_0_L2A` and
  `S2C_50TMK_20260928_0_L2A`, in an isolated store. Both tiles contributed to
  the output across their seam. The saved WGS84 polygon masked 126,581 pixels.
  Rasterio/GDAL and GEOS independently checked all 754,956 samples of the
  201 × 1252, three-band UInt8 GeoTIFF: every sample was exact, with EPSG:32650,
  10 m pixels, pinned source provenance and unchanged original source hashes.
  This was reuse of verified downloads, not a fresh network-download test.
  Output SHA-256:
  `0f8f175c3f7e51b42ce2f4e044faf5e6c81e705a0f5b6d4e9ef3a926ad49185e`.

The independent report is generated under
`.verification/acceptance/real-rgb-release-20260930/independent-verification.json`.
The QA dependencies are not included in, or required by, the product.

## Repeatable real-data acceptance

Run [the independent verifier](../../scripts/verify-real-rgb-mosaic.py) with a
store containing two completed real RGB jobs, their saved project, and a small
source-CRS rectangle spanning the tile seam. It rechecks source hashes, reuses
the exact originals read-only in a new isolated directory, exercises the real
project API and independently validates every output sample. Use a fresh output
directory and an unused loopback port. The verifier starts and stops its own
acceptance runtime; it never acquires or changes the source store.

For Windows distribution, [the installer verifier](../../scripts/verify-windows-installer.ps1)
runs the actual installer silently into a fresh `.verification/` directory.
It can replace a previous candidate and reinstall, checks every installed payload
hash, verifies extra-file and application-data retention on uninstall, then restores
the original installation registration and shortcut. It is a current-host test;
it does not claim clean-machine, interactive wizard or native GUI acceptance.

## Remaining public-release requirements

| Requirement | Evidence still needed |
| --- | --- |
| Native desktop workflows | Search, selection, both download types, project continuation, local inspection, processing, reveal/export, language and error recovery operated in the packaged window. |
| Fresh Windows host | Install and start with the documented WebView2 prerequisite; user-data retention through a supported upgrade/uninstall. |
| Release identity | Signing configuration, publisher identity, public version and final tagged clean source. Current candidate is unsigned. |
| Supported scope | Sentinel-2 SCL and true-color, same-grid WGS84 UTM processing; no claim of general reprojection, arbitrary multiband science, all planned providers or six-domain completion. |
| Website and GitHub | Matching current downloads/checksums, supported capabilities, help, privacy, support and update/rollback instructions. Publish only after release acceptance. |

The full product plan remains tracked by
[the release gates](../../GeoD-Global-Spec/06-Decisions-Contracts-and-Release-Gates.md).
