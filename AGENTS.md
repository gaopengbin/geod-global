# GeoD Global repository instructions

## Product and repository boundary

- This is an independent product repository. The domestic application is a separate project.
- Do not change the domestic checkout as a side effect of work here. Its code audit in `GeoD-Global-Spec/05-*` is reference material, not a runtime dependency.
- Do not resolve source code, binaries, or node_modules through a sibling checkout, absolute workstation path, junction, or fallback environment variable.
- Shared functionality may later use a reviewed, versioned package or library. Preserve origin, license notices, version and compatibility tests. Shared implementations do not mean shared app settings, updater channels or releases.
- Beautiful UI is the user-approved global design system. All pages use the shared `prototype/src/ui/` layer; do not introduce page-local generic widgets or a second theme. Beautiful UI source primitives and Radix/shadcn behavior adapters share one foundation. Keep GIS rendering and application logic as domain components. Full product scope is in the specification; a design prototype is not an implemented desktop capability.
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
