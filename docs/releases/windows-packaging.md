# Windows evaluation packaging

The release-preparation workflow produces a standalone desktop, CLI, examples,
English workflow documentation, third-party notices and a traceable manifest.
It does not install the application, sign binaries, publish releases or imply that
remote CI has run. The independent application ID is `xyz.laogao.geod.global`.

## Build after source freeze

Run from this repository on Windows x64 with its locked npm dependencies, the
MSVC Rust toolchain, Python 3.12+ and (for the optional installer) NSIS already
installed. The script does not install tools or require RTK.

```powershell
npm ci
npm run verify
cargo test --locked -p geod-runtime
cargo test --locked -p geod-global-desktop --features custom-protocol
python scripts/package-windows.test.py
./scripts/package-windows.ps1 -Profile release -Installer nsis
```

For only a portable ZIP, use `-Installer none`. Use `-Profile debug` only for an
explicitly labeled development/evaluation artifact. Both profiles embed a fresh
frontend via `custom-protocol`; the packaging entry point always builds the
frontend, desktop and CLI. It does not silently package arbitrary old executables.
The explicit x64 target uses `-Ctarget-feature=+crt-static` for this build process
only; no system environment variable is changed. PE imports are checked after
building, and packaging rejects remaining dynamic Visual C++ runtime imports.
The packaged CLI is also run with `--help` as a read-only JSON smoke check.

Output goes into a new `.verification/packages/GeoD-Global_...` directory. Each
name includes version, target, profile, Git revision, dirty status and UTC time.
Existing output directories are never erased or reused. Source files are hashed
before and after building; concurrent source changes stop packaging.

For a coordinated release, split the long binary build from the short final
package step. Documentation and QA notes may change while binaries compile:

```powershell
./scripts/package-windows.ps1 -Profile release -BuildOnly
# Finish release documentation, then briefly freeze package content.
./scripts/package-windows.ps1 -Profile release -Installer nsis -PackageOnly
```

The first phase freezes runtime/frontend sources, static assets, Tauri configuration,
permissions/icons and Cargo/npm manifests/lockfiles. It records their exact content
hash, including the embedded recipe schema, and both binary hashes in a build receipt.
Cargo's output directory is explicitly pinned to this repository's `target/`, even
if a caller has `CARGO_TARGET_DIR` or an outer Cargo configuration.
The second phase refuses changed
build inputs or binaries, verifies each copied executable against the receipt again,
then collects the latest documentation/licenses and
checks the full source tree throughout final packaging. An arbitrary prebuilt
executable cannot substitute for a completed receipt. The release manifest includes
the separate build-time and packaging-time source records. ZIP verification rejects
absolute, drive-prefixed, ambiguous and duplicate paths before validating its single
root manifest and every file hash.

The package directory contains:

- Portable ZIP, verified against every manifest file size and SHA-256.
- Optional `-setup.exe`, created from the same staged payload.
- `artifacts.json` and `SHA256SUMS.txt` for the outer artifacts.
- The staged payload for inspection, including `release-manifest.json`.

Use a clean tagged revision for a public release candidate. A dirty local package
is labeled as such and has a content hash of the actual source files; the commit
alone is never presented as its complete source provenance.

```powershell
python scripts/package-windows.py --verify-zip PATH_TO_PACKAGE.zip
Get-FileHash -Algorithm SHA256 PATH_TO_PACKAGE.zip
```

## Installer and user-data boundary

The repository's small NSIS script requests `user` execution level. Its default
installation is `%LOCALAPPDATA%\Programs\GeoD Global`, with only current-user
uninstall metadata and a Start Menu shortcut. It checks for an existing WebView2
runtime and does not download or install prerequisites. The portable ZIP likewise
requires [Microsoft Edge WebView2](https://developer.microsoft.com/microsoft-edge/webview2/).

Uninstall uses a generated list of packaged filenames, then removes empty package
directories. There is no recursive directory deletion. It does not remove the
application's LocalAppData/AppData, rasters, job history, recipes, preferences or
unrecognized files added by the user. An old installation's uninstaller clears the
shared shortcut and uninstall registration only if the registered install location
still belongs to it, preserving a newer installation at another location.
No installer or uninstaller is executed by
the packaging or static-test scripts.

Tauri's bundle configuration records the current-user NSIS intent but retains
`bundle.active: false`: the documented entry point above owns this exact payload
and retention policy. Do not enable Tauri's default installer as an equivalent
substitute without reviewing its app-data deletion UI. The official
[Tauri Windows guide](https://v2.tauri.app/distribute/windows-installer/) describes
current-user installation and WebView2 choices; its
[NSIS template](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi)
includes an optional delete-app-data path that this package deliberately omits.

## Licenses and source

The script inventories the resolved Rust graph and installed npm graph. It copies
license/notice texts from published dependencies and retains exact versions.
When a crate omits the license file, it uses the crate's recorded Git commit to
retrieve the official upstream text. For the explicitly documented upstreams that
also omit standalone texts, it pairs unmodified standard terms with their published
declarations and original source/copyright headers; these are marked in the inventory.
Packaging fails for any other uncovered dependency. Exact published MPL crate source is included, along with Inter's font license,
SCL legend attribution/license and sample-data provenance. This inventory also
includes build-time and optional dependencies; it is not a binary linkage claim.

First-party packages remain UNLICENSED/LicenseRef-Proprietary. The packaging
workflow does not decide public or commercial licensing. No private workspace
paths, credentials, user runtime data or sibling checkout files are copied into
the payload. Source provenance contains repository-relative paths and hashes.

## CI and remaining release gates

`.github/workflows/windows-artifacts.yml` is manual (`workflow_dispatch`) and has
read-only repository permissions. It verifies the code, builds an unsigned
portable package and uploads an expiring workflow artifact. It has no public
release/upload-to-distribution step, signing secret or updater key. A workflow file
existing locally is not evidence that GitHub ran it.

The local static checks cover ZIP integrity/tamper rejection, x64 executable
headers, PowerShell syntax and generated NSIS compilation with an explicitly fake
payload. The fixture installer is not an app build and must not be distributed.
Actual source-frozen package hashes and executable startup evidence belong in the
final acceptance record and generated manifests.

Public distribution still requires a chosen first-party license/distribution
policy, signing identity, clean-machine WebView2/startup/installation acceptance,
interactive uninstall-and-data-retention acceptance, and an explicit update policy.
