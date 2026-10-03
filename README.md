<div align="center">

<img src="docs/images/readme-cover.svg" alt="GeoD Global — geospatial data workspace. Windows development preview, GPLv3." width="100%">

### From scene to local data.

A Windows workspace for discovering imagery, inspecting source pixels,
preparing supported rasters, and keeping their provenance close.

**[Explore the workflow](#one-workflow-from-discovery-to-delivery)** · **[Build from source](#build-from-source)** · **[Share feedback](https://github.com/gaopengbin/geod-global/issues)**

English · [简体中文](README.zh-CN.md)

</div>

---

<img src="docs/images/sentinel2-desktop.jpg" alt="Actual GeoD Global Windows desktop showing Sentinel-2 imagery around San Francisco Bay, scene selection, acquisition details and geographic preview." width="100%">

<sub>Actual English Windows desktop capture, September 30, 2026. Copernicus Sentinel data (2026) · Earth Search · Natural Earth overview. This newer development UI is ahead of the public source snapshot; it shows a remote imagery preview, not completed download or export.</sub>

## Built for the handoff before analysis

Finding a scene is only the beginning. GeoD Global brings discovery, source files, raster inspection and repeatable preparation into a local project, so you can continue in your GIS or research tools with the data's context intact.

| Discover & compare | Inspect & prepare | Keep & repeat |
| :--- | :--- | :--- |
| Search supported Sentinel-2 scenes by area, date and cloud cover. Review metadata and compatible previews. | Read original SCL pixels, inspect a local raster and crop a rectangle on its source grid. | Keep source identities, recipes, provenance sidecars and SHA-256 checksums with your outputs. |

## One workflow, from discovery to delivery

**01 / Find a scene** → **02 / Get the source** → **03 / Inspect pixels** → **04 / Review a crop** → **05 / Export with context**

1. **Discover** Sentinel-2 through the supported Earth Search catalog. Check the footprint, date and source assets.
2. **Download** a supported full-scene SCL or true-color GeoTIFF, or a JPEG thumbnail. Tasks persist locally; interrupted downloads can restart from the beginning.
3. **Inspect** supported UInt8 SCL rasters in their original WGS84 UTM grid. The local 2D workspace supports up to four layers in the same CRS.
4. **Prepare** a rectangular SCL crop after preflight. Save and rerun the recipe through the workspace or CLI.
5. **Deliver** a verified, managed crop as GeoTIFF plus provenance, recipe, README and checksums in a ZIP. The current TIFF packaging limit is 32 MiB.

[Read the executable SCL workflow →](docs/workflows/clip-sentinel-scl.md)

## Current scope

**Public development source · Windows release in preparation.** The checked-in branch is an earlier development snapshot. Newer desktop work is being developed and verified separately; the screenshot above is labelled accordingly.

| Available in this source snapshot | Still outside this snapshot's scope |
| :--- | :--- |
| Sentinel-2 discovery and supported public-asset downloads | General multiband processing and arbitrary reprojection |
| SCL pixel inspection and source-grid rectangular crops | Polygon masking, general scientific analysis and 3D |
| Local projects, recipes, CLI and bounded MCP tools | Cloud synchronization and verified protected-provider workflows |
| English / Simplified Chinese, light / dark interface | Supported public installer, signing and clean-machine release acceptance |

The previous v0.1.0 preview release is a draft. Historical unsigned CI artifacts are evaluation builds. An official Global website and publicly hosted early-access form are being prepared; neither has a public URL yet.

## Build from source

Use **Node.js 22.12+**, **npm 10+** and **Rust 1.91.1+**. Start from this repository's root:

```sh
git clone https://github.com/gaopengbin/geod-global.git
cd geod-global
npm ci
npm run dev
```

Open **http://127.0.0.1:4317/** for the local development UI. To enable its supported real file downloads, run this in a second terminal:

```sh
npm run runtime
```

The runtime listens on `127.0.0.1:4318`; local tasks and files live in `.geod-global/`. Catalog browsing does not need a GeoD account. Data-provider rights and access requirements still apply.

<details>
<summary><strong>Native Windows desktop, build and verification</strong></summary>

See the [desktop setup](src-tauri/README.md) for platform dependencies. The native desktop calls the Rust core directly and does not need the separate browser runtime.

```sh
npm run desktop:dev
npm run desktop:build
```

`desktop:build` produces a debug executable, not a signed public installer. For production frontend preview:

```sh
npm run build
npm run preview
```

Contract checks also need Python 3.12. Use a virtual environment for its dependencies:

```sh
python -m pip install -r requirements-dev.txt
npm run verify
npm run test:runtime
```

`npm run verify:all` combines the frontend/contract checks and runtime tests; `cargo test --locked --workspace` includes the desktop crate. These are commands to run, not a claim that every current remote CI job has passed.

Full reference: [development guide](docs/development-guide.md) · [开发与验收参考](docs/development-guide.zh-CN.md).

</details>

## One core, several ways to work

**Desktop workspace** for hands-on preparation. **CLI** for repeatable recipes. **Local MCP** for supported tool calls. All use the same Rust task and raster core.

The MCP entry is `geod-runtime serve-mcp`. It starts with read-only tools and preflight; writes require explicit enablement. [Connection modes and lifecycle →](docs/mcp.md)

## Explore the project

| Start here | Go deeper |
| :--- | :--- |
| [SCL crop workflow](docs/workflows/clip-sentinel-scl.md) | [Raster inspection and language support](GeoD-Global-Spec/10-Languages-and-Raster-Inspection.md) |
| [Local MCP setup](docs/mcp.md) | [Processing and recipes](GeoD-Global-Spec/11-Executable-Processing-and-Recipes.md) |
| [Development guide](docs/development-guide.md) | [Workspace and delivery evidence](GeoD-Global-Spec/12-Workspace-Agent-and-Distribution.md) |
| [Windows packaging](docs/releases/windows-packaging.md) | [Product specification](GeoD-Global-Spec/00-README.md) |

## Feedback & contributions

Tell us about a recent task: which scene or raster you used, what you needed as local output, and where preparation became awkward. [Open an issue](https://github.com/gaopengbin/geod-global/issues) or [follow the developer on Bluesky](https://bsky.app/profile/laogao98.bsky.social).

For a code change, start with an issue to agree on its scope. Keep fixtures separate from real downloads and processing evidence, and retain third-party attribution. Leave private client data, credentials and sensitive coordinates out of public reports.

## License

First-party project code is **[GPL-3.0-only](LICENSE)**, © 2026 Gao Pengbin. Commercial use is permitted under the GPL; distribution of covered software must meet its corresponding-source and licensing requirements. [Separate commercial permissions may be discussed](COMMERCIAL-LICENSING.md); this is not an automatic exception.

Third-party software, fonts, imagery and other assets retain their own terms. [Screenshot and illustration provenance](docs/images/README.md).

---

<div align="center"><sub>GeoD Global · Independent desktop software by Gao Pengbin · Keep the source. Keep the context.</sub></div>
