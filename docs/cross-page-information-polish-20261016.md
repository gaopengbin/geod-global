# Cross-page information UI — 2026-10-16

The Explore information-window change now extends through Settings, My Data and projects, Tasks, Workspace raster/vector/tile inspectors, 3D assets, processing errors, monitoring area details and Agent model-connection/privacy help.

## One interaction rule

- Use the shared `InfoDetails` component and existing Beautiful UI tokens. An accessible information icon and tooltip sit beside the associated title or actions.
- Brief technical errors use a popover; provenance, license, file metadata, diagnostics and longer explanations use a dialog.
- These surfaces do not change the panel/card height. Escape and close restore focus. Metadata already inside a dedicated information window uses titled sections, including GeoPackage/Shapefile layers, classification legends and quality counts.
- Status and recovery actions remain visible. Failed task rows show a short one-line summary with full text available in the tooltip and details. Required download/processing confirmations, actual pixel results and operational forms retain their controls.
- Agent per-turn execution grouping, download groups, monitoring history, usage/order records and functional add/import/processing controls retain their existing behavior. They are operational content rather than explanatory folds.

The convention is also recorded in the repository instructions to keep later pages consistent.

## Implementation

Converted 36 remaining descriptive folds, consolidated Workspace scientific display explanations beside its inspector heading, grouped vector provenance in one action, removed unused card expansion containers, and moved settings source/network/account help beside their headings. Default-open release notes and GeoParquet properties no longer expand the page. Project service errors now show an actionable summary with technical details in a popover.

`InfoDetails` supports content styling independently from the trigger and embedded sections for information that is already inside a dialog. This keeps legends styled correctly after portal rendering and avoids nested explanation folds.

## Verification

- Full UI suite: 805 passed, 0 failed, 1 intentionally skipped (806 total).
- Final shared UI/project/runtime-file regression set was rerun after the last changes: 32 passed.
- Localization: 7 passed, including interpolation consistency and the new Chinese labels.
- Browser checks used the actual Settings and Tasks routes and a temporary read-only preview of 3 saved local files and 2 historical failed downloads. The preview did not run transfers, retries or model requests; source URL query strings were removed. The temporary preview files were removed before the production build.
- Settings height: 1048 px before/after opening diagnostics; dialog width 680 px; no inline explanatory fold.
- Task service banner: 48 px before/after opening technical details.
- Saved-file list: 224 px before/after opening provenance. Task list: 110 px before/after. No card expansion region remained.
- Dark mode and an 840 × 720 viewport were inspected. The 680 px task dialog fit without page/dialog horizontal overflow. Nested technical popovers closed independently of the task dialog.
- Screenshots and test reports: `.verification/all-page-info-20261016/`.

Frontend production build and native `custom-protocol` build passed. Before restarting the idle desktop, the check found zero active downloads and zero active Agent sessions. The rebuilt desktop launched successfully (PID 194060); its GeoD Global window was responding. Temporary browser tabs were closed and the viewport override was reset.

This is interface validation. It does not claim a new provider download or model acceptance run. Browser screenshots do not establish native visual acceptance.
