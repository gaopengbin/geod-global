# GeoD Global 0.2.0 — Windows desktop

GeoD Global brings source selection, map exploration, reviewed downloads and
local results into one Windows workspace. Local use and your own model
connection require no Global sign-in.

## What's included

- A classified picker with **133 directory entries across 17 groups**. These
  include products, protocols and local formats: **110 Ready, 2 Auth, 21 Planned**.
  Connector status is not a claim that every collection or file was tested.
- Compact result lists, multiple selection, map preview and a shared final
  download review. Source information and technical details use information
  windows. Downloads, validation and saved results retain their actual status.
- Catalog searches load the latest **100** results first. Additional pages load
  on demand; NAIP prefers the most recent acquisition year. Changing the area
  does not automatically enumerate the full historical catalog.
- A conversational Agent with your own model connection, task confirmation,
  persistent jobs and result inspection. Home conversations and Explore sidebar
  tasks retain their originating scene when manual exploration changes.
- Source-resolution-aware map zoom limits and compact download progress in chat.
- Anonymous usage statistics **on by default**, without account sign-in. Settings
  provides off controls and preserves saved opt-outs. Statistics exclude messages,
  credentials, exact locations, scene IDs, file paths and account identities.
  Development and production observations are separate. See
  [the data boundary](../usage-analytics.md).

## Download and install

Use the **Windows x64 setup EXE**, or extract the complete **portable ZIP**.
Microsoft Edge WebView2 Evergreen is required. Separate Node, Rust, Python or
system GDAL installations are unnecessary. Closing the window keeps the tray
process running; quit from the tray before upgrading.

The release includes exact first-party source and dependency notices. Verify
`SHA256SUMS.txt` and `artifacts.json`. Executables are **unsigned**; checksums do
not establish a Windows publisher signature. Automatic signed updates are not
enabled, so upgrades use a new downloaded installer.

## Supported scope

NASA, GSCloud and other protected sources retain their own account and product
requirements. Google Earth Engine uses your own project and may consume its
quota. Planned entries are not complete integrations. Source-specific limits,
licenses and numerical meanings stay visible in the product and documentation.
Previewed imagery, rendered service exports and original scientific bands remain
distinct delivery types.

Local data and supported processing remain on your computer. Data requests
contact their providers; model requests contact your configured model provider.
Hosted models, live paid services, cloud synchronization and general cross-grid
processing are not included in this release. Windows ARM64, macOS and Linux
installers are not provided.

First-party code is GPL-3.0-only. Dataset and component licenses remain separate.
Please report reproducible issues without keys, private project data or passwords.
