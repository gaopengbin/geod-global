<div align="center">

<img src="docs/images/readme-cover.svg" alt="GeoD Global — geospatial data workspace. Windows release candidate, GPLv3." width="100%">

### From scene to local data.

A Windows workspace for discovering imagery, inspecting source pixels,
preparing supported rasters, and keeping their provenance close.

**[Download candidate](https://github.com/gaopengbin/geod-global/releases/tag/v0.1.0-rc.3)** · **[Explore the workflow](#one-workflow-from-discovery-to-delivery)** · **[Build from source](#build-from-source)** · **[Share feedback](https://github.com/gaopengbin/geod-global/issues)**

English · [简体中文](README.zh-CN.md)

</div>

---

<img src="docs/images/sentinel2-desktop.jpg" alt="Actual GeoD Global Windows desktop showing Sentinel-2 imagery around San Francisco Bay, scene selection, acquisition details and geographic preview." width="100%">

<sub>Actual English Windows desktop capture, September 30, 2026. Copernicus Sentinel data (2026) · Earth Search · Natural Earth overview. This capture predates the current release candidate and shows a remote imagery preview, not completed download or export.</sub>

## Built for the handoff before analysis

Finding a scene is only the beginning. GeoD Global brings discovery, source files, raster inspection and repeatable preparation into a local project, so you can continue in your GIS or research tools with the data's context intact.

| Discover & compare | Inspect & prepare | Keep & repeat |
| :--- | :--- | :--- |
| Search supported Sentinel-2 scenes by area, date and cloud cover. Review metadata and compatible previews. | Read original SCL pixels, inspect a local raster and crop a rectangle on its source grid. | Keep source identities, recipes, provenance sidecars and SHA-256 checksums with your outputs. |

## One workflow, from discovery to delivery

**01 / Find a scene** → **02 / Get the source** → **03 / Inspect pixels** → **04 / Review a crop** → **05 / Export with context**

1. **Discover** Sentinel-2 through the supported Earth Search catalog. Check the footprint, date and source assets.
2. **Download** a supported full-scene SCL or true-color GeoTIFF, or a JPEG thumbnail. Tasks persist locally; interrupted downloads can be retried through the task page.
3. **Inspect** supported UInt8 SCL rasters in their original WGS84 UTM grid. The local 2D workspace supports up to four layers in the same CRS.
4. **Prepare** a rectangular SCL crop after preflight. Save and rerun the recipe through the workspace or CLI.
5. **Deliver** a verified, managed crop as GeoTIFF plus provenance, recipe, README and checksums in a ZIP. Ordinary crop bundles cap the TIFF at 32 MiB; documented scientific RGB bundles use a separate 512 MiB limit.

[Read the executable SCL workflow →](docs/workflows/clip-sentinel-scl.md)

## Current scope

**Windows 0.1.0-rc.3 release candidate · GPLv3 source.** Downloads and their actual publication status are listed on [GitHub Releases](https://github.com/gaopengbin/geod-global/releases). The candidate is for evaluation and feedback; the complete product plan remains in development.

| Included in this candidate | Remaining limits |
| :--- | :--- |
| Nine public source entries: Sentinel-2, Landsat, MODIS, radar, NAIP and DEM | Arbitrary science formats, reprojection and cross-grid processing |
| Supported original-pixel inspection, crop/mosaic, scientific RGB and documented quality filters | Other products and quality rules require separate implementation and validation |
| Named projects, task retry, persistent thumbnails, CLI and local MCP | Protected original downloads and an in-app Agent assistant remain pending |
| Branded desktop header, resizable panels, tray background tasks, English / Chinese and light / dark themes | Unsigned Windows x64; native GUI and clean-machine acceptance remain separate |

Choose the per-user installer or portable ZIP on the release page. Windows 10/11 x64 and WebView2 Evergreen Runtime are required; development tools are unnecessary. The files are unsigned, and this candidate is not the latest stable release. [Windows setup](docs/releases/WINDOWS-README.md) · [Candidate notes](docs/releases/0.1.0-rc.3.md).

The source menu distinguishes online previews, imagery loaded after selection and footprint-only catalogues. [Public preview checks](docs/explore-public-previews.md) · [Provider capabilities and original-file evidence](docs/providers.md). NASA Earthdata / Copernicus authorization setup is included; protected original downloads remain disabled pending real-account verification.

The old v0.1.0 release remains a draft. An official Global website and hosted early-access form remain in preparation.

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

The MCP entry is `geod-runtime serve-mcp`. It starts with read-only tools and preflight; writes require explicit enablement. The in-app conversational Agent is planned and is not included in this candidate. [Connection modes and lifecycle →](docs/mcp.md)

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
