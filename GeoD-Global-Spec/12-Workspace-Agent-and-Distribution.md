# Workspace, Agent and delivery progress — 2026-09-22

This is an implementation and verification record, not a declaration that the full overseas product is released. All work remains in the independent `G:\code\geod-global` repository.

## Implemented

- Real local 2D workspace using OpenLayers in the raster's original UTM CRS, with up to four same-CRS layers, visibility, opacity, fit, unload, keyboard navigation, rectangle selection and numeric source-coordinate bounds. Previews use nearest-neighbor display; pixel queries read and checksum the original file. Map layers are currently scoped to the open page session.
- A map selection opens the existing pinned-source recipe review. Native preflight is required before saving or running; no map interaction starts processing automatically.
- Exact source-grid pixel queries in the Rust core, HTTP service, Tauri command and MCP adapter. Right and bottom outer edges are excluded, matching the north-up pixel grid.
- Verified derived-output ZIP preparation: GeoTIFF, provenance sidecar, recipe, README and per-entry SHA-256 checksums. Only managed successful crop outputs qualify, TIFF inputs are capped at 32 MiB, metadata is checked against committed job records, and changed existing packages are never overwritten. The package includes user-defined recipe names and spatial bounds; review those before sharing.
- Local library search and original/derived filters; allowlisted support diagnostics containing versions, capabilities and aggregate job counts, with no automatic upload.
- Standard MCP stdio entry point using official `rmcp` 3.4.0: seven read-only tools by default and twelve with explicit `--allow-write`. Independent-store and loopback-service modes have distinct ownership and shutdown behavior. See [MCP setup](../docs/mcp.md).
- English and Simplified Chinese UI coverage for the added workflows.
- Windows portable/installer packaging scripts, a manual artifact-only CI workflow, third-party license collection and guarded source/payload hashes. These scripts are not evidence that a release installer was produced or installed.

## Verified evidence

- Clean `npm ci --no-fund` completed; npm audit reported zero vulnerabilities. Repository isolation, contracts, recipe schema, production frontend build and 32 JavaScript tests passed.
- `cargo test --locked --workspace`: 56 Rust tests passed (50 library, 3 CLI, 3 desktop). Workspace Clippy across all targets with warnings denied passed.
- Desktop review caught and fixed an outdated Tauri command ACL. All 16 registered commands now have matching generated and main-window permissions, without remote-origin access. An automated regression check rejects missing manifest/permission entries and remote expansion. The three desktop tests also passed with the production `custom-protocol` feature after this fix; formatting checks passed.
- Nine packaging tests passed, covering source-input selection, pinned Cargo output, executable identity, ZIP integrity/path rejection and installer boundaries. Packaging review also added post-copy binary checks against the build receipt and protection against an old installation removing a newer installation's shared registration. Actual installation/uninstallation remains a separate manual acceptance step.
- Source scene `S2C_10SEG_20250707_0_L2A`, local job `933dc541-ccaf-4e4b-8bf2-c0f2f9cadd6b`: 5490 × 5490 UInt8 SCL, EPSG:32610, 20 m pixels; original SHA-256 `ede35bce788bbafd2c0dbda4bca8c0b56c30fbf1027b37db63d8c5ee92e8b1d8`.
- Independent Rasterio/GDAL checks matched 12 exact pixel queries across the source and a derived raster, rejected four exclusive outer-edge queries, and verified ZIP integrity, exact TIFF/sidecar bytes, per-entry hashes and repeatable package generation. Script: `scripts/verify-workspace-delivery.py`; local evidence: `.verification/workspace-delivery.json`.
- Real MCP initialize/list/call/error/EOF behavior passed. A new MCP crop job `26119774-870e-4b97-9f27-7fa8e6b907cb` produced a 1020 × 895 GeoTIFF; all 912,900 output samples, georeferencing, source immutability and provenance passed independent Rasterio/GDAL comparison. Evidence: `.verification/mcp-processing.json` and `.verification/mcp-crop-independent.json`.
- Browser UI loaded source and derived layers, read the real center pixel (vegetation, value 4, column/row 2745), changed opacity to 50%, hid and restored a layer, and reviewed/saved/executed a source-CRS selection `[540000,4172000,555000,4186000]`. Job `2aa0529b-88c4-46b6-93bc-9ba68413dcc8` completed at 750 × 700 pixels. Independent Rasterio/GDAL comparison verified all 525,000 output samples, georeferencing and provenance; output SHA-256 is `918124d02f2f0c0e0cf55398b01542cdc086f460cd01e6a670ead6b121800621`.
- The delivery ZIP for that map crop contains 39,887 bytes with SHA-256 `1e611fcf927db2194bee6d3630ddba1669b52328cd0cce5f41012ef2886fa6b8`. The browser Download ZIP action saved it to the Windows Downloads directory; the downloaded file checksum matches the managed export. This verifies the HTTP ZIP download, not the older design-prototype blob download path.
- English/Chinese map labels, light/dark appearance, two-corner drawing, Enter-to-read-center keyboard behavior, local-library search/type filtering (including empty results), and diagnostics were inspected in the browser. At 390 × 844 the map, pixel inspector and recipe dialog were usable without horizontal page overflow. Default viewport and the original Chinese/light preferences were restored after verification. No browser warnings/errors were reported. Screenshot evidence is in `prototype/qa/workspace-*.png`; structured evidence is in `prototype/qa/workspace-verification.json`.

## Incomplete verification and product scope

The first optimized static-CRT Windows build hit an OS access-denied error launching an ICU build script. Host shell and browser tools subsequently stopped responding, then recovered. The build was restarted with two workers. No final installer or portable release is claimed by this record until its artifact checks are recorded below. Native desktop GUI/IPC, installation, uninstallation and clean-machine acceptance remain unverified.

The implemented raster operations remain bounded Sentinel SCL inspection and rectangular crops. General multiband/large-file processing, arbitrary reprojection, polygon masking, 3D, other providers, cloud/account synchronization, overseas payment activation, signing and public distribution remain future work within the full product scope.
