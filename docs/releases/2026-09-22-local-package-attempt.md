# Local Windows package attempt — 2026-09-22

Historical status at the initial attempt: **packaging implementation prepared; no
final desktop distribution package produced at that point**. This note preserves
the failed attempt; successful later packages are identified by their generated
`artifacts.json`, `SHA256SUMS.txt` and embedded `release-manifest.json`.

The independent application identifier remains `xyz.laogao.geod.global`. The
source-frozen packaging attempt used the current release script, embedded the new
frontend (`index-DiYTRY3P.js`, `workspace-map-rV_opi5c.js`) and began a Windows x64
release build with static CRT linkage. The build did not complete.

## Checks completed before the failure

- Five packaging unit tests passed: x64 PE header validation, ZIP file/hash
  validation, tamper/extra-file/traversal rejection, NSIS path escaping and
  uninstall deletion constraints.
- Python compilation, PowerShell parser validation and the installed Tauri
  configuration schema check passed.
- The license audit collected 383 resolved Rust/installed npm package notices,
  including OpenLayers and proj4. The audit reported no missing license text.
  Applicable original source archives, standard terms and source attribution are
  included by the packaging collector.
- NSIS compiled an explicitly marked **fixture-only** payload containing no GeoD
  executable. `7z t` validated that fixture installer archive. The fixture is a
  compiler test, not an application installer, and must not be distributed.
- Existing development executables import `VCRUNTIME140.dll` (and the desktop
  also `VCRUNTIME140_1.dll`). The release script therefore builds with an explicit
  x64 target and `-Ctarget-feature=+crt-static`, then rejects remaining dynamic VC
  runtime imports before creating a distribution.

## Observed blocker

The first release attempt failed to start a newly generated Rust build script:

```text
failed to run custom build command for icu_normalizer_data v2.3.0
could not execute process target/release/build/icu_normalizer_data-.../build-script-build
Access denied (os error 5)
```

Cargo then waited for other compilation jobs. The packaging session was
interrupted and exited with code 1. Independent read-only shell probes also
stopped returning; the parent task observed the same host-tool issue. The precise
host cause has not been established. No security setting, registry permission,
antivirus policy or system tool installation was changed.

The script now limits Cargo to two parallel jobs for the next attempt. Hanging
read-only tool calls were terminated; they did not change project data.

## Required continuation

After host execution recovers, inspect any residual processes belonging to this
failed release build. Preserve unrelated application/service processes. Re-run:

```powershell
./scripts/package-windows.ps1 -Profile release -Installer nsis
```

The default script rebuilds and compares source content before/after packaging.
The documented two-phase `-BuildOnly` / `-PackageOnly` path also requires a matching
completed binary build receipt; it cannot bypass source or binary verification.
Only a successfully completed run may produce final artifact hashes and a release
manifest. Then verify the portable ZIP's complete manifest, inspect/test the NSIS
archive without installing it, and run the packaged CLI's real workflow and a
bounded desktop startup smoke check.

No final ZIP hash, installer hash, signed binary, public release, remote CI result,
interactive installation or uninstall/data-retention acceptance is claimed by
this attempt. The separate unsigned-signing and clean-machine acceptance gates
remain open even after a later local package build succeeds.
