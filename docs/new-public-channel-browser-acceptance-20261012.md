# Five public products — actual Explore browser acceptance

2026-10-12. **Passed, five products.** The existing Explore dropdown was used in the Codex in-app browser against the local Windows native companion on ports 4317/4318. Each product was resolved against its published area/year, its actual publisher metadata was read, an original asset was explicitly selected and saved into a new project, and a fresh complete-original download was started through the UI. The completed local file, located map preview, raw pixel values, original source metadata and project reopening after a page reload were then checked.

The five additional browser downloads total **61,496,285 bytes**. They are separate from the isolated [18-original native acceptance](new-public-channel-native-acceptance-20261012.md). Actual browser observations are recorded in [the browser receipt](../prototype/qa/new-public-channel-ui-verification-20261012.json). A read-only native/file cross-check subsequently matched every browser-observed SHA-256, grid and pixel value against the five actual completed jobs and files. All five transfers were fresh, without resumed bytes.

| Product / chosen original | Browser-observed values and mask |
| --- | --- |
| Fractional Cover 2025 `pv_pc_50` | 3,200 × 3,200, EPSG:6933; (117, 2285) = 2, valid; embedded NoData = 255 |
| WOfS 2025 `frequency` | 3,200 × 3,200, EPSG:6933; (2021, 617) = 1, valid; (710, 897) = NaN, NoData; (0, 0) = 0, valid |
| WSF2019 `wsf2019` | 22,488 × 22,488, EPSG:4326; (9240, 14472) = 255, valid settlement; (0, 0) = 0, valid background; no embedded NoData |
| GMW2020 `mangrove` | 4,500 × 4,500, EPSG:4326; (483, 92) = 1, valid; (0, 0) = 0, NoData |
| CGLS2019 `permanentwater_cover_fraction` | 91,587 × 87,011, EPSG:4326; (59734, 39060) = 100, valid; (0, 0) = 255, NoData |

All pixel coordinates above are zero-based `(column, row)`. Preview stretching and map display did not change original file hashes or numerical values. The added EPSG:6933 display definition uses the native grid's ellipsoidal cylindrical equal-area projection; the actual geographic test coordinate also matched its native source cell. This does not add a general reprojection or derived science workflow.

The final dropdown contains **20 curated products and 15 imagery provider entries**. The source directory has **99 entries: 52 ready, 6 authorization required, 41 planned**, across 16 groups. Products, protocols and local formats are included; these totals are not platform or satellite counts. Each new product retains its documented regional/year boundaries. Only CGLS permanent-water fraction is enabled; the other 13 preserved publisher assets remain disabled with a clear unsupported-layer message. The original S3 `gls/` declaration and separately reviewed HTTPS `cgls_landcover/` download path remain visible.

Responsive checks at **768 × 800** passed for the Explore product panel and the original-raster dialog: no document horizontal overflow, and the dialog's client/scroll width was 728 pixels. The temporary viewport override was removed. Actual screenshots are retained under the ignored `.verification/public-datasets-ui-20261012/` directory, with their individual paths in the receipt.

Validation: 383 Node client tests and 463 full UI tests passed; a subsequent scoped CGLS wording correction passed its 34 UI tests, 28 client tests and production build. Native workspace tests, strict Clippy, formatting and repository/contract/recipe checks passed separately. Existing build chunk-size warnings remain. This is a local development/browser-companion acceptance, not native-window, installed-app or production-deployment acceptance. No credentials, model requests or Earth Engine compute requests were used for these new public-source tests.
