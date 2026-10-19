<div align="center">

<img src="prototype/public/brand/geod-symbol.png" width="80" alt="GeoD Global">

# GeoD Global
**Find geospatial data. Review the task. Keep the result.**

[![Windows](https://img.shields.io/badge/Windows-10%20%2F%2011%20x64-2563eb?style=flat-square)](docs/releases/WINDOWS-README.md) [![Release](https://img.shields.io/github/v/release/gaopengbin/geod-global?style=flat-square)](https://github.com/gaopengbin/geod-global/releases/latest) [![GPL](https://img.shields.io/badge/license-GPL--3.0-0f766e?style=flat-square)](LICENSE) [![Checks](https://github.com/gaopengbin/geod-global/actions/workflows/check.yml/badge.svg)](https://github.com/gaopengbin/geod-global/actions/workflows/check.yml)

**[Download 0.2.0](https://github.com/gaopengbin/geod-global/releases/tag/v0.2.0)** · **[Website](https://geod-global.laogao.xyz)** · **[Feedback](https://github.com/gaopengbin/geod-global/issues)**

English · [简体中文](README.zh-CN.md)
</div>

A Windows workspace for finding, previewing, downloading and inspecting geospatial data.
Explore yourself, or connect your own model and let the Agent prepare a reviewed task.
**Local use and your own model require no Global account.**

<img src="docs/images/sentinel2-desktop.jpg" width="100%" alt="Real native GeoD Global map workspace">

<sub>Historical native capture, September 30, 2026. Copernicus Sentinel imagery via Earth Search; remote preview, not a completed download. [Image provenance](docs/images/README.md).</sub>

## One path from source to result

| Choose | Explore | Confirm | Download | Inspect |
| :--- | :--- | :--- | :--- | :--- |
| Classified products and access conditions | Compact lists, multiple selection, map preview | Source, area, files and destination | Persistent jobs and explicit outcomes | Local files, provenance, checksums and source pixels |

- **133 directory entries · 17 groups:** imagery, terrain, thematic rasters, vectors, services and local formats. **110 Ready · 2 Auth · 21 Planned** describes connector status, not satellites or universally verified collections.
- **Latest 100 results first:** load more on demand. NAIP prefers the latest acquisition year; changing areas does not enumerate all historical data.
- **Bring your own model:** scoped native tools, task review and persistent jobs. Home conversations and Explore sidebar tasks retain separate selection contexts.
- **Source-aware results:** multiple-layer preview, raw-pixel inspection and supported compatible-grid crop/mosaic. Rendered exports and scientific originals stay distinct.

## Get started

1. Download the **Windows x64 setup EXE** or complete portable ZIP from [Releases](https://github.com/gaopengbin/geod-global/releases/latest).
2. Microsoft Edge WebView2 Evergreen is required. No separate Node, Rust, Python or system GDAL is needed.
3. Choose a source and area in Explore, search, select results and review the download. Connect your own model in Settings for Agent mode.

Files are **unsigned**. Check SHA256SUMS.txt and the release manifest; checksums are not a Windows publisher signature.
Quit via the tray before manual upgrades. Uninstall preserves user data. [Windows instructions](docs/releases/WINDOWS-README.md)

## Access, privacy and scope

Data-provider accounts and licenses remain separate. Protected sources need your own access.
Earth Engine uses your project and may consume its quota. Planned entries are not finished integrations.

Anonymous usage statistics are **on by default**, with off controls in Settings and saved opt-outs preserved.
They exclude conversations, keys, account details, precise locations, scene IDs and file paths. [Data boundary](docs/usage-analytics.md).
Model requests contact your configured provider; data requests contact the source.

This release focuses on **2D data**, including elevation rasters. Hosted AI, live paid services, cloud sync,
general cross-grid processing and non-Windows installers are not included.
[Source scope](docs/releases/source-scope.json) · [Provider status](docs/provider-integration-status.md) · [0.2.0 notes](docs/releases/0.2.0.md)

## Develop and contribute

Windows x64 development requires Node 22.13+, npm 10+, Rust 1.91.1+, Visual Studio C++ Build Tools and WebView2.

Run npm ci, npm run agent:prepare, then npm run desktop:dev from the repository root.

[Development guide](docs/development-guide.md) · [Agent architecture](docs/agent.md) · [CLI / MCP](docs/mcp.md) ·
[Processing example](docs/workflows/clip-sentinel-scl.md) · [Packaging](docs/releases/windows-packaging.md)

Suggest datasets and report reproducible problems through [Issues](https://github.com/gaopengbin/geod-global/issues).
Include product, period, steps and version; exclude credentials and private client data.

## License

First-party code: **[GPL-3.0-only](LICENSE)**, © 2026 Gao Pengbin. Commercial use is permitted under GPL obligations.
[Separate commercial permissions](COMMERCIAL-LICENSING.md) may be discussed. Datasets and components retain their own terms.
