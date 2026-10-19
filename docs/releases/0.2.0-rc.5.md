# GeoD Global 0.2.0-rc.5 — Windows Beta candidate

This local Windows 10/11 x64 candidate consolidates data selection, exploration,
download review and saved results. It is prepared for local acceptance before a
first public Beta. It is not a stable release or a public-release announcement.

## Included workflows

- One classified product picker, with channels grouped only when they deliver
  the same product. The directory contains **133 entries in 17 groups**:
  **110 Ready, 2 Auth and 21 Planned**. There are **132 picker products** because
  the two Sentinel-2 L2A COG channels share a product. Entries also include
  service protocols, data portals and local formats; these are not satellite or
  individually verified collection counts. The reproducible list is in
  `source-scope.json` in the source archive.
- Compact result rows and explicit selection. Product information, licenses,
  numerical interpretation and technical details use information windows.
- Shared selection actions, a final download review and task/result receipts.
  Online previews do not silently start full original downloads. A source that
  produces rendered maps, feature queries or subsets retains that delivery type;
  its output is not relabeled as an original satellite band.
- Agent task cards use the same review and delivery concepts. Home conversations
  and the Explore sidebar have separate selection contexts. Submitted tasks keep
  their reviewed source and area when manual exploration changes.
- Local downloads, task history, file inspection, source-pixel reading and
  compatible-grid processing retain their documented source and grid limits.
  Completed jobs must settle and verify before they appear as ready results.

## Evidence and limits

The October 18 development acceptance used a real configured model, renderer and
native engine to search, confirm and download fresh Sentinel-2 SCL files for San
Francisco and Paris. Both files passed local size/hash and native grid inspection.
Changing the manual Explore source and area did not replace the submitted tasks;
the Home run had no inherited Explore selection. A local hash proves local file
identity, not a publisher-supplied checksum. This acceptance does not prove every
source, full-size true-color downloads, or a fresh Windows installation.

NASA HLS, SRTM and reviewed VIIRS paths have real-account acceptance records;
users still need their own valid provider access. GSCloud uses its official login
window and session verification. Access can depend on each account and product.
Copernicus protected delivery and broad Planned families remain bounded by the
current provider documentation. A partial action under a Planned family does
not make that entire family complete. GEE exports use the user's own project;
quota consumption and eligibility remain the project owner's responsibility.

Data-provider access and model connections are separate from the Global account.
Users supply their own model connection. Official paid service, email delivery,
third-party identity consent and restore are not certified by local protocol
tests. Guest local workflows remain available. Cloud sync is not provided.

## Installation and support

The candidate includes a standalone desktop, CLI, reviewed Agent runtime,
documentation, exact first-party source archive and third-party notices. It
requires Microsoft Edge WebView2 Evergreen. No separate Node, Rust, Python or
GDAL installation is required. Files are **unsigned**; checksums do not replace a
publisher signature. Signed automatic updates are not enabled. Quit via the tray
before a manual upgrade. Uninstall is designed to retain user data and unknown
files, and still requires installation/upgrade/uninstall acceptance for this
candidate before a public Beta decision.

Source and build gates, real network tests, installer execution, native UI review
and user acceptance are separate checks. A successful archive verification alone
does not satisfy the latter checks. Report the version, reproduction steps and
safe diagnostic reference when reporting a failure; do not include credentials.
