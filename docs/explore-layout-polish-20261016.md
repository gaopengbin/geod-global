# Explore layout polish — 2026-10-16

The Explore sidebar now uses a 360 px initial width and a 340 px desktop minimum. The map's blank state fills its available panel and centers its guidance. Navigation defaults to the icon rail for new preferences, and entering Explore closes the Agent sidebar; explicitly opening the Agent remains a separate choice within that page. The scene inspector opens on demand.

Source selection uses the existing shared secondary button with an explicit border, database icon and chevron. Source resolution and preview notes are available through the information button. The classified source picker and source/account eligibility are unchanged.

Connected adapters share the same horizontal gutters, control heights and compact disclosures. Public products use a full-width layer selector with paired year/month fields and unboxed result rows. File provenance, licensing, measurement interpretation and original-source declarations remain available in details. Map loading and saving/downloading are separate explicit actions. The project destination form opens in a controlled shared dialog, preserving selection when closed and reopened.

## Verification

- 140 affected UI checks passed across source selection, shared Explore navigation, public-product selection and explicit delivery, and the public/service map components. Final per-file results: `.verification/explore-polish-20261016/final-tests.json`.
- Seven locale/interpolation checks passed; frontend and desktop builds passed.
- Browser checks covered light/dark themes, the 340 px sidebar minimum, 1280 px desktop layout, the normal 907 px viewport and a 760 px stacked layout. No document-level horizontal overflow was observed. At 1280 px, the map blank-state center differed from the panel center by less than 0.01 px.
- The real native JAXA MODIS shortwave-radiation connector returned a published September 2026 item for a public Beijing test rectangle. File selection and the destination dialog were checked against that metadata in an isolated QA runtime. No model request or original download was started for this layout check. This is browser-renderer evidence, not a native-window screenshot or a new transfer-speed acceptance.
- Representative vector-service controls were checked within the same sidebar. Existing service-flow checks cover the adapter boundaries; this task does not claim a fresh live download from every provider.
- The desktop was rebuilt only after verifying zero active Agent sessions and zero active transfer jobs.

Screenshots are saved under `.verification/explore-polish-20261016/`. Temporary browser/native bridge HTML was removed before the production build; the extra dataset bridge operations exist only in the opt-in test executable.
