# GeoD Global final-product readiness — 2026-09-28

The delivery target is the independent, supportable overseas product described
in `GeoD-Global-Spec/`, not a browser mockup or an SCL-only MVP. The desktop is
the local processing product; the public web presence must accurately explain,
document and distribute it. Cloud collaboration and paid products remain in the
full product plan and require their own release gates and explicit commercial
decisions before being offered. The current package is an **unsigned local
evaluation build**, not the final deliverable.

## Evidence from this build

- Build: Windows x64 release binaries, a portable ZIP and NSIS installer from
  clean source commit `199ed6fab3565aeed8b0b98b995bf49857904a92` after the
  Beautiful UI migration and packaging fix. The source tree SHA-256 is
  `b0702218368d50eb4c540a65c3bc05df0c2f039884f3951babd6cb2e003014c8`.
  Artifact record:
  `.verification/packages/final-product-eval-clean-20260928/GeoD-Global_0.1.0_windows-x64_release_199ed6fab3_20260928T093609Z/artifacts.json`.
- ZIP: 11,187,851 bytes, SHA-256
  `eeceb1807b6669a81ee85f43c0924cf7209c3ba72c12df1b520dbdcf38bb927d`.
  Every payload file passed manifest verification.
- NSIS installer: 7,134,752 bytes, SHA-256
  `ae90e48e9e52a32ba13102383aede3d9481d648d7be5e5b3f55515a4c21136e1`.
  NSIS compilation passed; the installer was not executed.
- The bundled `geod-runtime.exe` returned HTTP 200 from `/health` with an
  isolated temporary store. The same bundled CLI downloaded the public
  `S2C_10SEG_20250707_0_L2A` SCL asset (2,362,143 bytes), inspected it,
  planned/saved a pinned rectangle recipe and produced a real 1020 × 895
  GeoTIFF (46,315 bytes, SHA-256
  `af78deb054871d1f6b5c02de84f40fddbeac060ecd5d79781c305a7aa4063ecd`).
  The separate Rasterio/GDAL check passed all 912,900 samples, georeferencing,
  unchanged source, output hash/size and provenance sidecar. Local report:
  `.verification/acceptance/packaged-runtime-20260928T093228Z/independent-verification.json`.
  This end-to-end run used the earlier dirty-package CLI; its SHA-256
  `97dd8001eef872b5ac61085a798b6cd114746fd31398e1e137b297e7f8aca778`
  matches the CLI in the subsequent clean package exactly.
- Verification: `npm run verify` passed (32 domain tests, 8 UI tests and build);
  `cargo test --locked -p geod-runtime -p geod-global-desktop --features
  geod-global-desktop/custom-protocol` passed 56 Rust tests; the packaging suite
  passed 25 tests. The four missing npm license texts have explicit, reviewed
  collection paths and the package now contains a complete notice inventory.

## Gates still open

| Area | Current evidence | Required to claim a deliverable product |
| --- | --- | --- |
| Desktop GUI and IPC | Build and native command tests; packaged CLI end-to-end | Operate the packaged desktop GUI through search, download, inspect, processing, export and recovery on a real Windows machine. Verify WebView and IPC error states, keyboard access and language switching. |
| Windows distribution | ZIP manifest and installer compilation | Install, upgrade and uninstall on clean Windows with WebView2; verify preservation of user data, signing identity, update/rollback policy and clean revision provenance. |
| Data workflows | Real Sentinel-2 SCL download and rectangular crop | Implement and accept the specified imagery, elevation, vector and 3D workflows, source/provenance checks, metadata and format/CRS handling. Do not advertise a planned source or format as working. |
| Web and support | Local prototype and release documents | Publish a truthful product site, current English help/privacy/support materials and actual tested downloads only after the product and distribution gates pass. |
| Cloud / Teams | Design contracts only | Meet CLOUD-01–03 for account isolation, data lifecycle, consent, cost, operations and recovery before offering it. |
| Paid products | No merchant or price decision | Decide offers, prices, legal entity and merchant path; then meet PAID-01–04 with real transaction, entitlement, refund and reconciliation evidence before charging. |

`GeoD-Global-Spec/06-Decisions-Contracts-and-Release-Gates.md` defines the
authoritative desktop, cloud, paid and public-claim gates. The verified SCL
workflow establishes one real path, but does not satisfy the full data or desktop
GUI scope. A final release requires a fresh package and acceptance record after
the remaining implementation and checks.
