# GeoD Global desktop appearance

The current desktop shell is part of the product interface. This specification
continues the accepted Beautiful UI controls and blue theme; it does not
introduce a competing component system or claim platform certification.

## Primary references

- Microsoft's [Windows title-bar design guidance](https://learn.microsoft.com/en-us/windows/apps/design/basics/titlebar-design):
  combine app identity and context, keep window controls on the right, use 48px
  when including interactive controls, preserve drag/double-click/system-menu behavior.
- Microsoft's [application icon design guidance](https://learn.microsoft.com/en-us/windows/apps/design/iconography/app-icon-design):
  use one recognizable mark across the app's icon surfaces and check small sizes.
- [ArcGIS Pro's workspace structure](https://pro.arcgis.com/en/pro-app/3.6/get-started/user-interface.htm):
  map-first workspace, contextual actions and panes. GeoD keeps its simpler
  navigation and does not reproduce a ribbon or add unused features.
- [Tauri window customization](https://v2.tauri.app/learn/window-customization/):
  interactive controls must be separated from window dragging.

## Composition

| Area | Requirement |
| --- | --- |
| Title bar | One full-width 48px row; 12px outside padding, 8px within control groups, 16px between groups. |
| Brand | Original transparent blue/cyan GeoD A1 G, displayed at 32px. `Global` is subdued edition text. No alternate layers or green globe app mark. |
| Context | Project/workspace name then page, 13px, truncate long names. Text and separator only; no decorative folder tile resembling an action. Reserve flexible empty space for dragging. |
| Navigation | Collapse control at the left of the title bar; sidebar begins below it. Keep maps and file lists from losing a second title-bar height. |
| Caption | Plugin-owned minimize, maximize/restore and close-to-tray controls; Windows glyphs, 46 × 48px targets, no transforms affecting native hit geometry. |
| Appearance | Shared surface, text, hover and accent tokens. Theme also reaches native window chrome. Blue is the interface accent; class raster colors remain data. |
| Small windows | Minimum native window 900 × 680; hide redundant workspace status before it collides with caption controls. |
| Accessibility | Visible keyboard focus, localized caption labels including restore state, inactive-window distinction, high-contrast styles. Text at least 12px. |
| Motion | Continue existing shared motion tokens; caption hover transitions preserve fixed native hit targets. |
| Map feedback | Loading, slow-source and error panels reuse the shared Surface, Spinner, Progress and Button components. Match light/dark surface and text tokens, keep motion and actionable retry. |
| Fallback | If custom decoration cannot activate, restore/show the native frame. Startup failure must not strand an invisible background application. |

Theme switches and navigation buttons are not draggable. The project/context and
empty title-bar area are draggable. Right-click opens the actual Windows system
menu; the OS receives double-click maximize/restore. Caption close uses the
existing close-to-tray lifecycle.

Desktop activation waits for consecutive stable viewport/title-bar measurements
and shares any pending request between consumers. A native fallback receives one
new activation attempt after its restored client bounds settle. Persistent
failure retains the usable native frame instead of looping or disabling native
Snap geometry validation. This avoids making a transient resize leave two bars
for the rest of the session.

After initial startup, minimized or tray-hidden windows defer frame activation
without measuring their native caption rectangle or revealing the window.
Restoration/focus/visibility events resume activation after the client layout
settles. Native fallback can retry on a later focus event, but its own resize
events do not start a recovery loop. This also covers a development hot reload
while the window is minimized.

## Action hierarchy

Use the shared Beautiful UI components and tokens consistently within each
operation group. A component library alone does not establish action priority.

| Role | Treatment |
| --- | --- |
| Primary action | Blue filled button; at most one preferred action in a group, such as download, save or run processing. |
| Peer actions | Secondary outlined buttons with the same height, icon size and spacing. Repeated file/task-card actions use compact icon buttons with localized hover and keyboard-focus tooltips; keep labels on primary workflow actions. |
| Compact tools | Quiet icon buttons for small contextual tools, such as collapse, rename or close; always provide an accessible name. Card actions share one outlined treatment rather than mixing long text buttons and bare text. |
| Tabs | The shared navigation segmented control; active underline and label, distinct from an action button. |
| Information and references | Plain metadata or a clearly styled external reference link. Do not style inert folder icons or status labels as controls. |
| Card navigation | A project card may be the navigation target with an obvious hover/focus state; avoid repeating an extra Open button inside the same card. |

Content action buttons and detail triggers are 32px high with 13px labels and
16px icons. Repeated card actions use 32 × 32px targets with 8px spacing and
no permanent text labels. Their accessible names and shared Radix tooltips
describe the destination or action; tooltips dismiss with Escape and avoid
viewport edges. Keep related actions in one aligned footer, wrapping as a
group when the card is narrow. Use the same treatment in light and dark themes.

## Content composition

- Use a compact title beside the page's tabs. Do not stack a title, generic
  description, duplicate navigation button and tabs above a data collection.
  Library navigation already supplies Explore; project-specific continuation
  remains in project details because it preserves the current project.
- Collection titles, tabs and filters scroll together with the content. Do not
  pin an isolated heading above a scrolling collection or cover file cards.
  Saved clipping settings belong in the clipping dialog for their pinned source,
  with a fresh processing check required after applying them. They are parameters,
  not files or projects, and have no separate section in My Data.
- File cards have a 96px preview column spanning the complete summary height;
  metadata and a 32px icon strip share the adjacent column. The image covers
  this navigation thumbnail without changing the original raster. Expanded
  provenance occupies a separate region, leaving the preview and action strip
  stable. A closed disclosure must reserve no blank space.
- Project cards use the same full-height media layout. Project details combine
  asset labels and readiness counts, then file headings and tabs, rather than
  repeating totals and generic guidance on separate lines.
- Task cards keep status, transfer information and actions in one compact
  summary; errors and technical details expand below it. Settings align labels
  with their fields and share one persistence note for immediate preferences.
- Use 12px between content groups and 16px top inset on desktop collection
  pages. Check dense collections as well as a few records, at 900 × 680,
  1024px and larger widths, in both languages and themes. No text below 12px.

## Panel widths

- The navigation/work area, Explore imagery list/map/details, and Workspace
  layers/map use shared draggable separators. Side panes retain pixel widths
  during window resizing when space permits; the map fills the remaining space.
- Separators have a subtle line and centered grip, blue hover/drag/focus feedback,
  and a larger pointer hit region. Arrow keys adjust the focused separator;
  double-click restores the group's default widths.
- Remember manually adjusted widths locally, separately for Explore and
  Workspace and for their visible pane combinations. Navigation retains its
  expanded width when collapsed to the 72px icon rail and opened again.
- Enforce readable minimum widths rather than allowing panes to disappear.
  Existing close/show controls still release and restore space. Below 761px,
  the browser keeps the existing stacked layout instead of horizontal resizing.
- Map viewports update as the separators move. Narrow map panes wrap their
  toolbar; the timeline remains a single 80px strip with centered dates and
  scene counts and horizontal scrolling. Layer selection lives in the map
  toolbar when multiple visible scenes are loaded, without a duplicate caption.

## Explore filters and selection

- Open filters in a 600px dialog rather than squeezing a scrolling form into
  the scene list. Group area, UTC date range and cloud range with consistent
  field widths and spacing. Keep draft changes local until search is applied;
  invalid ranges remain in the dialog with an actionable validation message.
- Use the shared DayPicker/Radix date picker, including styled month/year
  selection, keyboard navigation and exact ISO calendar-day form values.
  Calendar dismissal restores focus without dismissing the enclosing dialog.
- Keep the selection/loading/project actions hidden until at least one scene
  is selected. The list heading retains an accessible bulk-selection icon.
- Remove the global bottom workspace/account status strip. Dataset attribution
  stays beside the map it describes.

## Acceptance before public release

Check the real desktop window as well as the shared browser interface:

1. Light/dark, English/Chinese, 900 × 680 and normal/wide layouts: no caption
   collisions, horizontal overflow or duplicate OS/application headers.
2. Drag, double-click, maximize/restore, minimize/restore, keyboard focus,
   right-click/Alt+Space system menu and close-to-tray/single-instance restore.
3. Windows taskbar, tray, executable and installer use the GeoD G icon.
4. Windows 11 Snap hover and 125%/150% display scaling need separate native
   checks; source inspection or a browser screenshot cannot close these checks.
5. Frontend activation failure must retain a usable native frame.

Desktop preview needs both the catalog and the reviewed Sentinel COG origin in
`connect-src`; `img-src` does not authorize TIFF range requests. GeoTIFF decoding
uses bundled blob workers, so `worker-src` permits `self` and `blob:` while
`script-src` continues to prohibit remote, inline and eval scripts. The repository
checker and regression tests cover these distinct boundaries. This permits map
preview requests; it does not make them follow the original-file download proxy.

Current native evidence and remaining checks are recorded in
[the desktop appearance acceptance record](../releases/2026-09-30-desktop-appearance-acceptance.md).

A title-bar fix is not proof that all data workflows, whole-product visual quality,
installation or publishing gates are complete. The
[product-quality review](../releases/2026-09-30-product-quality-review.md) keeps
those outstanding items visible.
