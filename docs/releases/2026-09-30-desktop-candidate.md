# Windows desktop candidate — 2026-09-30

Distribution target: the product website and GitHub downloads. Microsoft Store,
accounts, subscriptions and cloud collaboration are outside this candidate.
This is release preparation, not authorization to publish the build.

## Changes and current acceptance

- The desktop is the primary development entry point. Its own Rust manager
  performs local work through 27 scoped IPC commands; no companion HTTP service
  is required by the desktop.
- Repeated application launches restore the existing window before trying to
  open the exclusive store. The official single-instance plugin is pinned to
  2.4.5, compatible with the existing pinned Tauri 2.11.6 runtime.
- The window's close button hides it to the native system tray. Rust downloads
  and processing keep running; clicking the tray restores the same window.
  The tray has Open, View tasks and Quit and stop tasks actions, synchronized
  with the interface's English/Chinese language. Background behavior is also
  explained in desktop Settings. No frontend tray/window/shell permissions are
  granted.
- Explicit Quit stops accepting new IPC commands, drains in-flight command
  writes, interrupts unfinished jobs, cancels their workers and waits for
  temporary-file cleanup and durable job records before exiting. Reopening
  offers explicit retry from the start, preserving completed files and prior
  user cancellations. Automated tests cover running and queued transfers,
  queued clipping/mosaics, write failure, repeated exit and reopening. Closing
  to the tray never invokes this shutdown path. Native tray clicks and an actual
  hidden-window transfer remain interactive acceptance items.
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

## Previous candidate package and actual Windows acceptance

The earlier clean-source candidate from commit `4417caa7b8` is retained under
`.verification/packages/GeoD-Global_0.1.0_windows-x64_release_4417caa7b8_20260930T105806Z`.
It predates the tray changes described above and is an unsigned local evaluation.

| Artifact | SHA-256 |
| --- | --- |
| NSIS installer | `97b8c3adb05311da5895f275a5f5f8521a41a05749bfb1a5599250b3cd457680` |
| Portable ZIP | `3e3631514fbb7734026fa305e37e09bd06a1393c2211c3570ea25fad933f1193` |
| Desktop executable | `cd5eccc03fec82e7d0d60d63396d273496ecf5f653025e1fcef7766a2073f247` |

The actual current-host NSIS test replaced the previous 0.1.0 candidate binary,
reinstalled, and uninstalled into an isolated path containing spaces. All 854
payload hashes matched. An extra canary file and all 490 existing application-data
files were preserved. The real installation's registration and shortcut were
restored exactly. This was same-version binary replacement, not a tested version
upgrade or a clean-Windows installation. Report:
`.verification/acceptance/installer-20260930-v2/installer-verification.json`.

The packaged desktop created a responding main window. A repeated executable
launch exited successfully with no stderr while preserving the original window
and one application process. Report:
`.verification/acceptance/desktop-launch-20260930.json`.

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
| Native desktop workflows | Tray hide/reopen/quit and a hidden-window transfer; search, selection, both download types, project continuation, local inspection, processing, reveal/export, language and error recovery operated in the packaged window. |
| Fresh Windows host | Install and start with the documented WebView2 prerequisite; user-data retention through a supported upgrade/uninstall. |
| Release identity | Signing configuration, publisher identity, public version and final tagged clean source. Current candidate is unsigned. |
| Supported scope | Sentinel-2 SCL and true-color, same-grid WGS84 UTM processing; no claim of general reprojection, arbitrary multiband science, all planned providers or six-domain completion. |
| Website and GitHub | Matching current downloads/checksums, supported capabilities, help, privacy, support and update/rollback instructions. Publish only after release acceptance. |

The full product plan remains tracked by
[the release gates](../../GeoD-Global-Spec/06-Decisions-Contracts-and-Release-Gates.md).
