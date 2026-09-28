# GeoD Global UI system

Beautiful UI is the global visual system, following the user's decision to replace the previous page-local component implementations. All eight routes share the same component boundary, typography, surface colors, borders, radii and interaction states.

## Source and ownership

- `prototype/src/ui/foundation.css`: Beautiful UI's pinned foundation, including light/dark tokens and Tailwind v4 utilities. Local font assets are retained for offline desktop use.
- `prototype/src/ui/index.jsx` and `styles.css`: upstream-derived components and application adapters. Beautiful UI supplies the button, sidebar and task presentation; shadcn/Radix fills the underlying form, modal, selection and disclosure primitives.
- `third-party/`: exact upstream source references, original source snapshots, license texts and adaptation notes. No commercial Central Icons package is included; iconography uses the existing Lucide dependency.
- Page styles contain domain layout and GIS presentation. They may position a shared control but must not introduce a second button, input, dialog or theme implementation.

The library is distributed as source. Keeping reviewed component source and thin compatibility adapters inside the repository is intentional; business screens import those components rather than implementing their own generic controls.

## Application integration

| Surface | Shared components |
| --- | --- |
| Application navigation and header | SidebarNav, Button, Badge, Toast |
| Explore | SegmentedControl, Input, Select, Slider through Input, EmptyState |
| Workspace | Shared forms, switches/checkboxes, modal and disclosure; OpenLayers owns geographic rendering |
| My Data and Tasks | Controlled TaskRows, Progress, Surface, Disclosure, action buttons |
| Recipes | Modal, form controls, source/plan surfaces, table and action buttons |
| Sources | Shared table, status badges and actions |
| Cloud | Shared surfaces and badges around proposed-service content |
| Settings | Select, Switch, Button, diagnostic controls |

`data-theme` and the root `dark` class switch the same foundation. All user-facing strings remain in the existing English/Chinese translation flow. Date/number localization and source identifiers remain unchanged.

Task status and progress come from the existing local runtime. Upstream animated demonstration task transitions and scripted agent responses are not used. Unknown processing totals remain indeterminate. Application source/catalog simulations retain their existing explicit labels.

## Adding or changing UI

1. Reuse the shared UI exports first. Add a reviewed upstream primitive or a documented adapter there when a new control is needed.
2. Pass translated labels, business data and handlers from the caller. Do not import application services into reusable primitives.
3. Use the foundation tokens. Keep scientific raster class colors distinct from decorative interface colors.
4. Keep source attribution and distributable notices current when vendoring upstream code.

`npm run check:isolation` also checks the UI boundary: raw generic controls and duplicate generic control implementations cannot be added to business JSX, and page styles cannot redefine global palette tokens.

## Verification

- `npm run verify`: repository/contracts checks, existing domain tests, shared-control interaction tests, production bundle.
- `npm run test:ui`: form submission values, localized/empty selections, modal focus and busy behavior, slider/checkbox keyboard behavior, exclusive segments, and indeterminate progress.
- Browser acceptance covers all routes, English/Chinese, both themes, desktop and narrow widths, actual dialog/select interactions, source search and existing runtime views. Automated DOM tests do not replace visual checks.

This migration changes the interface system. It does not add new providers, raster operations, account/payment services or native installer acceptance.

### Acceptance record — 2026-09-22

- `npm run verify` passed: repository/contract/recipe checks, 32 domain tests, 8 shared-control interaction tests and production build. Windows packaging tests passed all 23 cases, including vendored UI notice integrity and inclusion.
- A separate empty-directory install with npm 11.19.0 passed (199 packages, no reported vulnerabilities). This was Windows with Node 25.1.0; it is not evidence of a Node 24/Linux CI run.
- The in-app browser rendered all eight routes. English/dark pages were reviewed at desktop width; narrow navigation and layout were checked at 390px in English/dark and Chinese/light. Long select values truncate, dropdowns retain their full text, and map actions wrap without horizontal page overflow.
- Live Earth Search returned seven scenes for the existing Bay Area June 2025 query. Local data search reduced six files to one source; a source raster loaded on the map and returned class 4 (Vegetation) at its centre pixel. Clip review and recipe JSON dialogs opened, and Escape restored focus to the JSON action.
- The preview was returned to Chinese/light on Explore. No release was published and no new native installer acceptance was performed. The build retains a warning for the approximately 522 kB main JavaScript chunk (164 kB gzip).
