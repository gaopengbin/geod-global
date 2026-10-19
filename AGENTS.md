# GeoD Global repository instructions

## Product and repository boundary

- This is an independent product repository. The domestic application is a separate project.
- Do not change the domestic checkout as a side effect of work here. Its code audit in `GeoD-Global-Spec/05-*` is reference material, not a runtime dependency.
- Do not resolve source code, binaries, or node_modules through a sibling checkout, absolute workstation path, junction, or fallback environment variable.
- Shared functionality may later use a reviewed, versioned package or library. Preserve origin, license notices, version and compatibility tests. Shared implementations do not mean shared app settings, updater channels or releases.
- Beautiful UI is the user-approved global design system. All pages use the shared `prototype/src/ui/` layer; do not introduce page-local generic widgets or a second theme. Beautiful UI source primitives and Radix/shadcn behavior adapters share one foundation. Keep GIS rendering and application logic as domain components. Full product scope is in the specification; a design prototype is not an implemented desktop capability.
- Explanations, licenses, provenance, scientific display notes and technical error details use the shared `InfoDetails` information button. Place it beside the relevant heading or actions: short details use a popover, long content uses a dialog. Do not expand this material inside work panels or cards. Inside an information window, use plain titled sections instead of nested explanatory folds. Keep actionable status, required confirmation and functional controls visible; preserve Agent execution/history grouping.
- Product typography uses the shared scale in `prototype/src/ui/foundation.css`: page titles 20/28, panel titles 14/20, controls and lists 13/20, conversation text 15/25, and auxiliary text at least 12/18 px. Reuse the type and weight tokens; do not add page-local 10/11 px functional text. Preserve the Inter and system Chinese font stack, gray scrollbars, existing brand colors and responsive panel behavior.
- Product density uses the same foundation: page padding 12px by 16px, panel padding and section gaps 12px, control gaps 8px, panel headings 40px, and standard controls 32px. Combine related titles, statuses and actions in one row when space permits; wrap naturally on narrow panels. Do not add blank header rows, shrink readable text, or compress the map canvas to disguise whitespace.
- Never represent fixture data, task simulations or JSON reports as real raster processing or downloads.

## Commands

- Install from the root with `npm ci`. The single root package and lockfile own all frontend dependencies.
- `npm run dev`, `npm run build`, `npm run preview` use the source in `prototype/`; no npm workspace symlinks are required.
- Install contract dependencies with `python -m pip install -r requirements-dev.txt` (prefer a virtual environment).
- `npm run verify` checks dependency isolation, real fixture hashes, draft contracts and the production build.
- The preview listens on `127.0.0.1:4317` only. Do not expose it publicly or publish a Git remote without task authorization.
- Ignore generated dependency/build/test output; keep source fixtures and provenance in version control.

## Windows

- Use UTF-8 for text. Prefer `curl.exe` or `Invoke-RestMethod` for HTTP; if using `Invoke-WebRequest`, include `-UseBasicParsing`.
- On this user's Windows host use the configured `rtk` command wrapper. Do not require RTK in portable npm scripts or CI.
