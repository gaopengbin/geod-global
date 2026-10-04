# Desktop appearance acceptance · 2026-09-30

This records local Windows desktop checks in the independent `geod-global`
repository. It is an appearance/interaction pass, not whole-product release
approval. No public release or new installer was produced.

## Implemented

- One 48px title bar containing the GeoD G mark, navigation toggle, project/page
  context, theme control and platform window controls. The sidebar begins below it.
- Existing Beautiful UI tokens and controls remain the only application design
  system. Caption controls share the theme while preserving native hit targets.
- The original GeoD symbol and its source notice are bundled; PNG/ICO hashes,
  transparency, 16/24/32/48/64/256px ICO entries and favicon wiring are checked.
- Awaited custom decoration activation, native-frame fallback and an 8-second
  reveal watchdog. Only the main local window has the narrow command permissions.
- Map loading/error panels use shared Surface/Spinner/Progress/Button components
  and theme tokens, including animated loading and retry.
- Corrected the packaged WebView CSP: the previously missing Sentinel COG
  `connect-src` origin blocked actual metadata requests. Local blob decoder workers
  are allowed separately; remote scripts, inline scripts and eval remain prohibited.

## Native checks

| Check | Evidence and scope |
| --- | --- |
| Drag | Actual window origin moved from (72,155) to (177,176) in the previous pass. |
| Double-click maximize/restore | Actual header double-click expanded to 3440×1392 and restored to 1442×972 captured bounds. Restore label changed accordingly. |
| Minimize/restore | Sky reported the window minimized; activation restored the same window. |
| System menu | Header right-click and Alt+Space displayed the actual Windows move/size/minimize/maximize menu. |
| Edge resize | Right/bottom borders resized the actual window to 902×680 captured bounds with the configured 900×680 minimum. The first attempts started inside the client area and did not resize; the outer border did. |
| Minimum layout | Chinese and English title bars retained branding, context, theme and caption controls with no overlap. This does not approve every content pane at this size. |
| Close-to-tray and reopen | PID 60136 stayed alive after close, its main window disappeared, and launching the same executable restored window 728586 in the same PID with the same minimum size and page. No active download/processing job existed during this check. |
| Navigation | Header collapse/expand and navigation links operated in the real desktop window. |
| Language | English selection localized caption controls to Minimize/Maximize/Hide to tray and the shared pages. |
| Theme | Switched the actual rebuilt desktop between light/dark appearance. The title bar, caption controls, navigation, imagery list, details and timeline followed the same theme; the real COG remained visible. Restored Chinese/light appearance for local review. |

Screenshots and the final executable identity are retained locally in
`.verification/acceptance/desktop-appearance-20260930/`. The screenshots are real
native-window captures, not design mockups. The before-fix console screenshot
`cog-policy-before.jpg` shows the blocked `connect-src` request.

## Automated checks

- `npm run verify:all`: 65 client + 29 UI + 80 runtime + 4 desktop tests passed
  (178 total), with repository isolation, exact desktop ACL, brand asset and CSP
  checks, contracts and production frontend build.
- `npm run desktop:build`: rebuilt the debug executable with the bundled shared
  frontend and current CSP. The existing >500KB entry-chunk warning remains.
- `git diff --check`: passed.
- `cargo clippy --locked --workspace --all-targets --features geod-global-desktop/custom-protocol -- -D warnings`: passed.

## Remaining gates

- Actual Windows 125%/150% DPI and Windows 11 Snap flyout acceptance. The host's
  global display settings were not changed. WebView zoom is not a DPI substitute.
- Injected native activation/startup failure and its visible frame fallback;
  current fallback logic has source and UI integration coverage, not a native
  failure-injection run.
- Tray menu interaction and continued download/processing while hidden. The
  successful idle close/reopen check does not certify running-task recovery.
- All pages/dialogs at minimum dimensions and both languages/themes. Full
  multi-scene download, RGB/SCL mosaic, polygon clip, export and installer checks
  remain in the [product-quality review](2026-09-30-product-quality-review.md).

## Rebuilt native COG result

The rebuilt debug executable is 47,166,464 bytes, SHA-256
`133e8ae0a6af7560474d6ad96bdc8d2c02d27f53fc7ec79bb66a749145996e27`,
with frontend entry `index-Bp7uNmjf.js`.

In the actual desktop window, selected and loaded Earth Search scene
`S2A_10SEG_20260912_0_L2A`. The georeferenced true-color TIFF rendered in the map,
metadata loading completed and the loading overlay cleared. The retained
`native-normal-en-dark.jpg` shows that result. This exercises actual remote COG
preview and decoding in the packaged WebView; it is not an original-file download
or a full-resolution export. No jobs or new project records were created.

The final Chinese light/dark map captures are `native-normal-zh-light.jpg` and
`native-normal-zh-dark.jpg`. The local acceptance JSON includes screenshot hashes
and the executable hash so these captures can be tied to this reviewed build.

## Development title-bar recovery · 2026-10-01

The user's two-row screenshot showed the native Windows caption above the app
header. The running development log recorded `NativeApply: snap maximize-button
geometry lies outside the window client area`. The original one-shot activation
could leave this fallback in place after a transient layout change.

Activation now waits for stable viewport/app-header dimensions, shares pending
requests and retries a returned native fallback once after the restored client
bounds settle. It keeps the plugin-owned caption controls, geometry validation
and persistent-failure fallback. Page headings/tabs are outside this change.

The existing development process (PID 26996, main HWND 2885200) loaded the change
without a restart or installer build. Its main-window client origin was 1px below
the outer window top, rather than a native caption-height inset. Temporary
frontend diagnostics confirmed `custom` mode, an active decoration plugin, a
48px app header and maximize bounds `(1348, 0, 46, 48)` inside a `1440 × 970`
viewport. These diagnostics were removed after verification. The `WS_CAPTION`
style bit alone is not a valid visible-caption test for Tao's undecorated windows.

The desktop-frame/session UI suites passed 13 tests, including temporary fallback
recovery, bounded persistent failure, shared activation, browser behavior and
existing close-to-tray/quit handling. `npm run check` passed. Read-only native
capture returned a blank image for the occluded window, so it supplies no new
visual acceptance evidence. This check does not certify Windows DPI/Snap flyouts
or repeat the earlier mouse/keyboard interaction checks.
