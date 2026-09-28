# Beautiful UI source attribution

Upstream: https://github.com/slev12397/beautiful-ui

Pinned commit: `c99a3586cf4fc093091feb47d3c066da1fb2e342`.

License: MIT, Copyright (c) 2026 Shane Levine. The complete license is in `LICENSE`.
`provenance.json` records each unmodified upstream snapshot, source URL and SHA-256.

GeoD Global adaptations are in `prototype/src/ui/`:

- `foundation.css` takes the actual `:root`, `.dark`, `@theme inline`, and shared animation blocks from `app/globals.css`. It adds the existing `data-theme="dark"` selector and uses locally available fonts. Gallery backgrounds and unrelated demo styles are excluded.
- `index.jsx` preserves the upstream `Button.tsx` variant classes and CVA implementation, adding native props, Radix Slot composition, icon sizes and compatibility adapters.
- `SidebarNav` derives the upstream `RailButton` layout and styles, with application-supplied routes, labels, brand and Lucide icons. Demo workspace/account/chat menus and all commercial Central Icons imports are excluded.
- `TaskRows` derives the upstream task row, semantic status badge, detail and progress presentation. All demo records, simulated transitions and `useTick` timers are removed. Application job state is the sole source of status, progress and actions.

The raw registry foundation is retained for evidence, but the application uses the verified source CSS blocks: registry extraction can include unrelated component fragments. No Next.js, analytics, signup endpoint, remote font download, commercial icon package or upstream demo backend is bundled.

The component layer is owned locally after copying. Upstream updates require reviewing source and license changes, refreshing provenance hashes, and re-running accessibility, keyboard, form and build checks.
