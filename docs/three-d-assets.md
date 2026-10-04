# Public and owned 3D scenes

Settings → **3D sources** provides a public-source and local-scene entry.
My Data → **3D assets** lists saved scenes, with view, export and source-detail
actions. Opening a scene verifies its saved dependencies before rendering; the
renderer and its supporting files are bundled locally. No Cesium ion account or
CDN is configured.

For a public source, enter an explicit tileset/glTF/GLB URL, inspect its entry,
then declare the permission, attribution and optional license reference. Saving
requires the user's explicit permission confirmation and the exact inspected
entry hash. Inspecting a URL does not verify its license or confer download
rights. The optional Cesium demonstration is fetched only after user action;
it contains generated sample buildings, not production geography. Its
[upstream generator notice](https://github.com/CesiumGS/3d-tiles-samples-generator/blob/main/README.md)
describes the sample-data license.

The desktop's local-file entry accepts tileset JSON, glTF, GLB or a GeoD 3D ZIP.
Relative model, buffer, PNG/JPEG and nested tileset dependencies are copied from
the chosen entry's folder. Browser preview accepts the GeoD export ZIP through
the same native importer, without receiving native filesystem paths.

## Implemented scope

- Explicit 3D Tiles 1.0/1.1, including nested tilesets, multiple contents and
  b3dm contents; glTF 2.0 JSON and GLB model entries; relative or same-origin
  public dependencies; PNG/JPEG textures and buffer/schema resources.
- Structural and dependency checks, exact source hashes/sizes, entry discovery
  pinning, whole-scene persistence, offline display, camera manipulation and
  offline ZIP import/export. Missing or incompatible resources reject the save.
- Original transforms, bounds, glTF fields and binary/table bytes remain intact.
  URI localization is a separate view of the source graph. Renderer-scoped Blob
  resources accept only this scene's verified dependencies; the source's
  `asset.tilesetVersion` is preserved, while the renderer's Blob cache query is
  removed only from the local request address.
- The native core is shared by seven desktop commands, protected loopback HTTP
  routes and the `three-d` CLI group. Saved resource reads and exports recheck
  the entire relevant original byte identity. The CLI supports an explicitly
  named file or export path; the desktop uses user-operated file dialogs.

Acquisition currently saves the **whole source**. No area-of-interest selection,
tile intersection, mesh clipping, level-of-detail reduction or scientific
analysis is applied. Format/dependency checks are bounded structural checks,
not a complete glTF/3D Tiles conformance or geometry-quality certification.

Each package is limited to 128 MiB, 256 resources, 32 MiB per resource and 32
dependency levels; each tileset document is limited to 4,096 tiles. The registry
holds at most 128 packages. Public acquisition uses the configured native proxy,
public HTTPS on one origin, bounded transfer time and reviewed DNS policy;
redirects and query credentials are rejected. Local dependencies must stay
inside the selected folder and cannot use network paths, absolute paths or
filesystem redirections.

Implicit tiling, i3dm/pnts/cmpt, point clouds, terrain, 3DGS, Draco/KTX2 and other
unreviewed extensions are rejected. glTF's reviewed extension subset is
`KHR_materials_unlit`, `KHR_texture_transform`, `KHR_mesh_quantization`,
`EXT_mesh_gpu_instancing`, `EXT_mesh_features` and `EXT_structural_metadata`;
this is not a claim that every extension or metadata semantics has received
real-data acceptance. HTML credits are rejected to avoid unrecorded external
requests. Cross-origin, signed or authenticated 3D services, a transfer task
queue/cancel/resume, cache maintenance, preview thumbnails and full project
processing remain open work. Commercial/Google/Cesium-hosted asset entitlement
is not established by this increment.

## Export and import integrity

The export has `manifest.json`, `originals/<resource-id>.<extension>` and
`scene/<resource-id>.<extension>`. Original members are byte-identical source
files. Scene members rewrite only verified dependency URIs; transforms,
metadata, buffers, textures and table contents keep their original values.

Import checks the manifest receipt, every original hash/size, dependency graph
and tile count, every precisely reconstructed localized member, and complete
archive membership. It rejects changed/missing/extra members, links and unsafe
paths. It saves the **unchanged originals**, not the localized scene members.
Therefore a subsequent export still contains the same original model/texture
bytes and dependency locators. A new local-archive record retains the source
resource origin, previous receipt, source and rights declarations. Repeated
imports retain at most eight provenance levels; exceeding that bound rejects
the import. A receipt is an integrity record, not a supplier signature or a
license verification.

## Real acceptance

The independently checked public inputs are:

| Input | Original resources / bytes | Independent mesh triangles | Actual behavior |
| --- | --- | --- | --- |
| [Cesium nested tilesets](https://raw.githubusercontent.com/CesiumGS/cesium/main/Specs/Data/Cesium3DTiles/Tilesets/TilesetOfTilesets/tileset.json) | 8 / 51,124 | 600 | Seven tiles, nested source graph, b3dm/GLB geometry, original/export comparison and offline viewer |
| [Khronos BoxTextured](https://raw.githubusercontent.com/KhronosGroup/glTF-Sample-Assets/main/Models/BoxTextured/glTF/BoxTextured.gltf) | 3 / 8,285 | 12 | External binary and PNG texture, rendered textured model, import/export source preservation |

Both are upstream demonstration assets. BoxTextured has CC-BY-4.0 and a separate
[Cesium logo/mark notice](https://github.com/KhronosGroup/glTF-Sample-Assets/blob/main/Models/BoxTextured/README.md);
it is QA data, not bundled application content. Every public resource was also
retrieved independently and compared with the native saved bytes. All declared
geometry/metadata and non-URI fields in localized export members were compared.

Headless acceptance uses actual Rust HTTP storage and Microsoft Edge software
WebGL at 900/1024/1440 pixels, English/Chinese and light/dark themes. It verifies
rendered geometry/texture, changing frames for drag/wheel/reset, complete
resource reads, canvas disposal, card/nav geometry, no remote asset requests and
the production renderer under the exact desktop CSP. The production test
bridges only the loopback browser adapter through the test harness; it does not
emulate an installed Tauri WebView or count native file-picker operation as
accepted. Source receipts and precise accepted stages are in the
[3D acceptance record](../prototype/qa/three-d-verification.json).

Actual native ZIP round-trips imported both public samples twice. Each copy
retained all original resource locators, hashes and bytes; its subsequent export
retained every original and localized payload member. The manifest changes to
record the new package and its prior source receipt. A local import of the actual
BoxTextured files also retained all three originals after its fresh test input
folder was removed. These checks use the downloaded demonstration bytes, rather
than synthetic parser fixtures.

Offline readback uses a restarted isolated runtime with its acquisition proxy
set to an unavailable loopback endpoint. An actual public discovery request was
rejected under that policy. The CLI then restored ten saved packages and read
all 45 resources, without changing the registry, original bytes or modification
times. This condition blocks the application's source-acquisition route; it does
not disable the host computer's network. The renderer test separately rejects
and records any browser request outside the local preview.

The final production renderer check covers both public scenes and the saved
local textured model at all three widths, after removing the local input and
blocking acquisition. Card metadata, viewer instructions and attribution use
the shared readable text color; their actual computed text/background contrast
is checked against 4.5:1 in both themes. No original byte, registry or file
modification time changes during this read-only renderer acceptance.

The renderer's local decoders need WebAssembly compilation. Desktop CSP adds
only `wasm-unsafe-eval` for that operation; JavaScript eval, inline/remote scripts
remain prohibited. The exact source/worker restrictions are checked statically
and the production browser acceptance separately probes local Wasm compilation
and blocked JavaScript `Function` execution through an ordinary same-origin
script. The narrower keyword follows the
[browser platform definition](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Content-Security-Policy/script-src#unsafe_webassembly_execution).

## Reproduce in isolated storage

Build the native runtime first. Keep its direct store closed while running CLI
acquisition. Use a separate folder from any user's desktop data:

```sh
python -X utf8 scripts/verify-three-d-public.py --data-dir .verification/3d-demo/root --output .verification/3d-demo/public
```

For the external-texture case, add `--source` with the BoxTextured glTF URL,
`--min-resources 3`, and its actual `--license`, `--attribution` and
`--license-url`; do not apply the default Cesium generated-data rights to it.

Start the native service for this isolated root at 4380 and either development
or production preview at 4317. Then run:

```sh
node scripts/verify-three-d-ui.mjs --server http://127.0.0.1:4380 --output .verification/3d-demo/ui
node scripts/verify-three-d-ui.mjs --server http://127.0.0.1:4380 --production --archive-roundtrip --output .verification/3d-demo/production
```

`--capture-engine-state` adds developer-only diagnostic instrumentation to Vite
source, never to production code. The default headless channel is installed
Microsoft Edge; another installed channel can be selected with `--channel`.
No user's browser profile or desktop window is operated. Stop the isolated
service before actual archive/local-file round-trip and CLI offline readback:

```sh
python -X utf8 scripts/verify-three-d-roundtrip.py --data-dir .verification/3d-demo/root --nested-evidence .verification/3d-demo/public --texture-evidence .verification/3d-demo/textures --output .verification/3d-demo/roundtrip
python -X utf8 scripts/verify-three-d-public.py --offline --data-dir .verification/3d-demo/root --output .verification/3d-demo/offline
```

The round-trip output must be fresh; the script removes only its own newly
created, validated test-input folder. After restarting the service,
`--production --local-scenes` adds its retained local model to renderer
acceptance. This acceptance produces no installer and publishes nothing.
