# Contributing to GeoD Global

Use Issues for reproducible bugs and source suggestions. Include the app version,
workflow and public product details. Exclude credentials, signed URLs, private
locations and customer files. 中文反馈同样欢迎。

Follow [AGENTS.md](AGENTS.md), the existing UI foundation and the independent
Global product boundary. Install dependencies from the root package. Run
`npm run verify:all` for a release change; focused tests are appropriate for a
small change. Packaging requires clean versioned source and exact file hashes.

Source contributions need documented access, license, scientific units, NoData,
coverage and actual transfer/inspection evidence. A catalog response, fixture or
rendered preview is not proof that original downloads work. Keep provider limits
visible and avoid promising untested collections.

This repository uses GPL-3.0-only for first-party code. Preserve third-party
notices and asset provenance. Review the [development guide](docs/development-guide.md)
and [source status](docs/provider-integration-status.md) before implementing a connector.
