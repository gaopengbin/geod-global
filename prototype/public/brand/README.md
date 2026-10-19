# GeoD Global brand assets

The simplified **Global v6** symbol uses the blue → cyan → green gradient from
the GeoD Global 1.1 brand kit. Its revised transparent PNG SHA-256 is
`01d4f478243768226237aedac44290665b48be3920e5b9117df17d7de0a3c7fe`.
The existing GeoD MIT notice remains in `LICENSE`.

Only selected production exports are included here, not the full design kit:

- `geod-symbol.png`: 256 × 256 transparent mark, used in the application header,
  Agent welcome view, sign-in and favicon.
- `global-space-v1.webp`: clean half-Earth space background from the same kit,
  used by the sign-in page without an embedded wordmark.
- `PROVENANCE.json`: selected kit exports and their SHA-256 hashes.

`src-tauri/icons/icon.png` is the same approved 256 px mark. `icon.ico` uses
the kit's transparent Windows export with 16/24/32/48/64/128/256 px resources
for the executable, window and tray. The CSS wordmark uses the kit's
`#087BEA → #00BBDF → #21C885` gradient while keeping the existing interface
control colors.

To reproduce, copy `icons/png/symbol-256.png`,
`icons/windows/geod-global-symbol.ico` and
`backgrounds/clean-space-original.webp` from
`design/brand/geod-global-brand-kit-1.1` to their corresponding paths above.
`scripts/verify-brand.mjs` pins the selected assets and verifies PNG
transparency and Windows icon sizes before packaging. There is no runtime
dependency on the design directory or the domestic product.

The 1.1 symbol retains the clean G without the globe or satellite orbit. Earlier kits are retained for comparison.
