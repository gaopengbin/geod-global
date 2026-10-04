# Development guide

[Product overview](../README.md) · [完整中文开发与验收参考](development-guide.zh-CN.md)

This guide describes the current 0.1.0-rc.2 source and its shared desktop/runtime core. Exact product support and real-data acceptance are recorded in the [provider guide](providers.md) and [candidate notes](releases/0.1.0-rc.2.md).

## Prerequisites and commands

Use Node.js 22.12+, npm 10+, Rust 1.91.1+ and Python 3.12 for contract checks. Work from the repository root; `prototype/` does not own a separate npm package.

| Command | Purpose |
| --- | --- |
| `npm ci` | Install the pinned root frontend dependencies |
| `npm run dev` | Local UI at `http://127.0.0.1:4317/` |
| `npm run runtime` | Rust file/task service at `127.0.0.1:4318`, store `.geod-global/` |
| `npm run desktop:dev` | Native Tauri development shell |
| `npm run desktop:build` | Debug executable without a release bundle |
| `npm run build` / `npm run preview` | Build and preview frontend production resources |
| `python -m pip install -r requirements-dev.txt` | Install contract/schema-check dependencies, preferably in a virtual environment |
| `npm run verify` | Isolation, fixture hashes, draft contracts, recipe schema, frontend tests and build |
| `npm run test:runtime` | Rust runtime tests |
| `npm run verify:all` | Both verification groups |
| `cargo test --locked --workspace` | Workspace and desktop tests |

The browser UI needs the runtime for supported file downloads. Its API accepts only configured loopback origins and mutations. Ordinary assets are capped at 512 MiB, with separate reviewed limits for large NAIP, radar, SAFE and SRTM products. After a restart, unfinished tasks can be retried; eligible public originals can resume from verified partial records, while other tasks restart from the beginning. See the [provider guide](providers.md) for the exact transfer and authorization rules.

The native shell calls the Rust core directly and stores data under the independent app ID `xyz.laogao.geod.global`. It does not need a separate HTTP runtime. See [desktop platform dependencies and acceptance](../src-tauri/README.md).

## Implemented boundaries

- Supported live catalogues share WGS84 region selection, pagination and product-specific filters. Optical scenes use UTC dates/cloud cover; radar, composites, aerial and elevation products have their own filters. Fixture catalogues and thumbnails remain separate from live-query evidence.
- Downloads validate size, signature and SHA-256. SCL inspection additionally reads georeferencing, decodes values and class counts; neither is classification-accuracy certification.
- Local inspection supports the documented RGB, SCL, reflectance, vegetation-index, quality, radar, NAIP and elevation formats. It returns original values and supported spatial metadata. The application runtime does not require system GDAL or Python; independent scientific QA uses separately pinned tools.
- The local 2D workspace allows four same-CRS rasters, visibility, opacity, fit, unload and source-pixel inspection. Layers belong to the current open page session; no online basemap or raster reprojection is performed.
- Rectangular crop preparation uses original source-grid coordinates and actual preflight. The output retains source pixels, CRS and resolution and has a provenance sidecar. Recipes can be saved, reviewed and rerun in the UI or CLI.
- Only verified managed outputs qualify for delivery ZIPs. Ordinary crop bundles cap the TIFF at 32 MiB; documented scientific RGB bundles use a separate 512 MiB streaming limit. Packages contain GeoTIFF, source manifest, recipe, README and per-file hashes.
- Support diagnostics are generated on demand, contain allowlisted versions/capabilities/counts, omit personal paths, source URLs and geographic coordinates, and are not uploaded automatically.
- `geod-runtime serve-mcp` exposes the shared local tool layer with reads/plans enabled by default and mutations explicitly enabled. Independent-store and existing-loopback modes have different task/lifecycle ownership. The [MCP guide](mcp.md) lists current tools and acceptance limits; an in-app conversational Agent is not yet included.

Supported project crops, polygon masks, aligned mosaics, scientific RGB and quality selection are product-specific. Vector, map-service, offline-tile and bounded 3D workflows have separate documentation and acceptance. Arbitrary formats, general reprojection, cloud synchronization and protected-provider original download acceptance remain incomplete. A design or schema draft does not establish a runnable capability.

## Repository layout

```text
prototype/            Shared desktop frontend and labelled design preview
  src/                React UI and styles
  public/             Real scene fixtures, thumbnails and font notices
  qa/                 Development review screenshots and evidence
crates/geod-runtime/  Rust tasks, raster core, recipes, delivery, CLI and MCP
src-tauri/            Native desktop shell using the same Rust core
schemas/              Executable recipe schemas
examples/             Actual request and fixed-input recipe examples
docs/                 Workflows, integration, development and packaging
scripts/              Isolation, fixture and packaging checks
GeoD-Global-Spec/     Full product specification and draft contracts
.github/workflows/    Independent checks and artifact/release automation
```

This is an independent product. Domestic GeoD is a separate repository and is not a runtime dependency. Do not resolve source, binaries or node_modules from sibling checkouts or local fallback paths. Future shared components need explicit versions, compatibility checks and original notices. See [repository boundary](../GeoD-Global-Spec/08-Repository-Boundary.md).

## Evidence and release status

- [Languages and raster inspection](../GeoD-Global-Spec/10-Languages-and-Raster-Inspection.md)
- [Processing and recipes](../GeoD-Global-Spec/11-Executable-Processing-and-Recipes.md)
- [Workspace, Agent and delivery](../GeoD-Global-Spec/12-Workspace-Agent-and-Distribution.md)
- [Windows evaluation artifact record](releases/2026-09-22-windows-artifact-acceptance.md)
- [Windows packaging and third-party notices](releases/windows-packaging.md)
- [CI and release automation](releases/automation.md)

Read the dated evidence for the exact tested build, operation and remaining gates. Packaging and process-start checks do not replace native interaction, clean-machine install/uninstall or signing acceptance. Current unsigned candidate downloads and publication status are listed on [GitHub Releases](https://github.com/gaopengbin/geod-global/releases). The old v0.1.0 release remains a draft.

## Licensing and assets

First-party code is [GPL-3.0-only](../LICENSE); [commercial licensing](../COMMERCIAL-LICENSING.md) is a separate inquiry route. The npm root's `private: true` prevents accidental package publication and does not make GitHub source private. Third-party notices remain applicable.

Fixture sources, URLs and hashes are recorded in `prototype/public/samples/manifest.json`. Inter font notices ship with the project; Chinese text uses platform fallback fonts. Review [README image provenance](images/README.md) for the dated native screenshot. Distribution packages include the exact committed project source archive, its hash and GPLv3 terms; [Windows packaging](releases/windows-packaging.md) describes the source-freeze checks.
