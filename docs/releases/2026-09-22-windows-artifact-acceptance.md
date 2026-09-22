# Windows local evaluation artifacts — 2026-09-22

The portable ZIP and NSIS installer were built successfully from clean commit
`ddace13472bac5a0185fdd70500e4d4d2fd5305d` in the independent GeoD Global repository.
They are unsigned local evaluation artifacts, not a public or signed release.

The local output directory is:

```text
.verification/packages/GeoD-Global_0.1.0_windows-x64_release_ddace13472_20260922T103329Z/
```

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `GeoD-Global_0.1.0_windows-x64_release_ddace13472_20260922T103329Z.zip` | 10,940,167 | `4659c37c17697c5a953d6630764f7ed1476114f990268f6d7f620161bd852c8a` |
| `GeoD-Global_0.1.0_windows-x64_release_ddace13472_20260922T103329Z-setup.exe` | 7,059,198 | `4ba2791c186ce54ec4d7a0e1a8acb7b42156a38812d04cde0de33c8de203378a` |

The packaging source tree SHA-256 is
`2a6709c615f06a3003818d584d3ea4107aa3f827d544f59cbb35e0d722d4dcaf`.
`artifacts.json`, `SHA256SUMS.txt` and `GeoD-Global/release-manifest.json` record
the exact inputs, compiler, build flags, binary checksums and individual files.
The output directory is intentionally ignored by Git; use the
[packaging workflow](windows-packaging.md) to rebuild it from source.

## Completed acceptance

- Both binary and package source receipts refer to the clean implementation
  commit. The final desktop includes the corrected ACL for all 16 local commands.
- The ZIP's full manifest was checked. The real NSIS archive passed `7z test`;
  all 669 extracted payload files matched the same manifest. The installer was
  never executed.
- Static-CRT x64 builds have no direct `VCRUNTIME` or `MSVCP` DLL imports. The
  observed imports are Windows system DLLs; this inspection does not substitute
  for clean-machine runtime acceptance. WebView2 Evergreen remains a prerequisite.
- Dependency notices cover 383 resolved Rust/npm packages, including applicable
  MPL source archives. The inventory distinguishes the resolved dependency graph
  from a claim that every listed dependency is linked.
- Authenticode checks identify the binaries and installer as unsigned.
- The **packaged** CLI completed a new-store real public Sentinel download,
  plan, recipe save/reload, crop run, list/status/inspect and deterministic rerun.
  Every command returned JSON stdout with exit code zero. The output is
  1020 × 895, 46,315 bytes, SHA-256
  `af78deb054871d1f6b5c02de84f40fddbeac060ecd5d79781c305a7aa4063ecd`.
  Independent Rasterio/GDAL comparison passed all 912,900 samples,
  georeferencing, unchanged source, output size/hash and provenance sidecar.
- The desktop executable from the actual staged payload remained alive and
  responsive during a five-second process smoke check, with zero stderr bytes.
  Only the process created by that check was stopped.
- Application tests: 32 JavaScript plus 56 Rust. Packaging tests: 9. Total:
  **97 unique tests**, plus the desktop cases repeated with production
  `custom-protocol`. Formatting, strict workspace Clippy and browser workflow
  acceptance also passed as documented in [implementation evidence](../../GeoD-Global-Spec/12-Workspace-Agent-and-Distribution.md).

Local raw reports: `distribution-verification.json` and
`installer-archive-test.txt` in the artifact directory;
`.verification/packaged-cli-e2e/acceptance.json`,
`.verification/packaged-cli-e2e/independent-rasterio.json`, and
`.verification/desktop-release-startup.json`.

## Remaining release acceptance

Native desktop GUI/IPC interactions, installation, upgrades, interactive
uninstallation/user-data retention and a clean Windows machine were not tested.
Static ACL checks and process startup are separate from those checks. Signing,
automatic updates, public publication and remote CI runs are also uncompleted.
The full product's other planned capabilities remain in the product specification.

This successful run follows the preserved [initial failed build attempt](2026-09-22-local-package-attempt.md).
