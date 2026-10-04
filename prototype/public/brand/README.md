# GeoD brand assets

The blue/cyan **G** is the existing GeoD A1 symbol, copied without alteration
from `GeoD-logo/03-geod-a1-symbol.png` in `gaopengbin/tif-downloader` at source
commit `0938626b16aa2662df6058b2a7bd7733c72a3f14`.

`geod-symbol.png` is a 1254 × 1254 transparent PNG. Its SHA-256 is
`ad28ec7751fc8d6ae9abae81038ad0ad408a642439bc34d26528189b95958000`.
The original repository's MIT notice is included in `LICENSE`.

The Tauri 2.11.5 CLI resampled this image to generate the desktop/tray PNG and
multi-size Windows ICO in `src-tauri/icons`. It did not redraw or recolor the
mark. The header, favicon and About view use this same source; the executable,
tray and installer use the generated application icons.

Reproduce from this repository root:

```sh
npx tauri icon prototype/public/brand/geod-symbol.png --output .verification/brand-icons
```

This is a checked-in brand resource, not a build or runtime dependency on the
domestic product. `scripts/verify-brand.mjs` checks the source and generated
icon hashes, PNG headers and Windows icon sizes before packaging.
