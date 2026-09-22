# GeoD Global for Windows

This is an **unsigned local evaluation package** for Windows 10/11 x64. It has
not been publicly released by this build script. Windows may display an unknown
publisher warning. Check the provided SHA256SUMS.txt and release-manifest.json
before running files from a source you trust; a checksum is not a digital signature.

## Start

1. Extract the entire portable ZIP into a writable folder.
2. Ensure the [Microsoft Edge WebView2 Evergreen Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)
   is installed. It is an external prerequisite, not bundled here.
3. Start `geod-global-desktop.exe`. No Vite, Node.js, Rust, Python or companion
   HTTP server is needed for the packaged desktop.

The optional NSIS installer uses the same files. It installs for the current user
under `%LOCALAPPDATA%\Programs\GeoD Global`, adds a Start Menu shortcut and an
uninstaller entry, and never requests an all-users installation. It checks for
WebView2 but does not install tools or prerequisites.

The portable folder is portable **application code**, not a portable data store.
Desktop jobs, rasters, recipes and preferences use the independent application
data directories for `xyz.laogao.geod.global`. Runtime records and managed rasters
are under `%LOCALAPPDATA%\xyz.laogao.geod.global\runtime`. Language/workspace
preferences live in the application's WebView data storage. Keep these directories
when moving or backing up user data. Do not run two copies against one runtime store.

Uninstall removes only packaged files, the app's shortcut and its uninstall entry.
It preserves application data and any extra files you added to the installation
directory. Deleting the portable folder also leaves application data intact.

## CLI and real raster workflow

`geod-runtime.exe --help` prints the command list as JSON. The CLI requires an
explicit `--data-dir` or an already-running loopback `--server`; it does not borrow
the desktop store automatically. Use a separate directory while the desktop runs.

See `docs/workflows/clip-sentinel-scl.md` for downloading a public SCL scene,
pinning the source, reviewing a real crop plan, saving/running the recipe and
inspecting the output. In this package, skip its source-build step and set the
executable path to `./geod-runtime.exe`; sample files are in `examples/`.
`docs/runtime.md` describes the supported raster formats, limits and API.
`docs/mcp.md` covers local Agent/MCP integration, and `schemas/` contains the
versioned recipe JSON schema shipped with this build.

The current executable path supports Earth Search Sentinel-2 catalog queries,
local transfers, real SCL inspection and rectangular GeoTIFF clipping. Screen
sections explicitly marked as design simulations do not become implemented
processing features by being packaged.

## Provenance and limitations

`release-manifest.json` records the application ID, package version, Git commit,
dirty-source status, source content hash, compiler version, build arguments,
unsigned Authenticode status and every bundled file's SHA-256. `THIRD-PARTY/`
contains dependency license texts, an inventory and applicable MPL source archives.
First-party licensing status is described in `FIRST-PARTY-NOTICE.txt`.

Signing, automatic updates, public distribution, clean-machine installation and
interactive uninstall acceptance are separate release gates. The packaging script
does not claim they passed. It never runs the installer or uploads an artifact.
