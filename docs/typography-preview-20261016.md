# Typography preview — 2026-10-16

The first typography pass uses the existing Beautiful UI foundation and GeoD branding. It consolidates 570 size declarations into shared tokens across the product styles, raises 10/11 px interface text to 12 px, reduces conversation prose from 16 to 15 px, and normalizes line heights and weight levels.

| Role | Size / line height | Weight |
| --- | --- | --- |
| Page title | 20 / 28 px | 600 |
| Panel or section title | 14 / 20 px | 600 |
| Controls and list titles | 13 / 20 px | 400–500 |
| Conversation and composer | 15 / 25 px | 400 |
| Auxiliary text | 12 / 18 px | 400 |
| Code and logs | 13 / 20 px | 400, existing monospace stack |

The font stack is centralized in `prototype/src/ui/foundation.css`: bundled Inter for supported Latin characters, Microsoft YaHei UI / Microsoft YaHei on Windows, and platform fallbacks. Browser platform-font inspection confirmed that the actual mixed-language conversation paragraph used Inter and Microsoft YaHei UI. No external font service or new font asset is needed.

Markdown headings use the conversation scale instead of accumulating proportional sizes. Regular inputs and primary controls align to 32 px height; compact Explore inputs use 13/20 px text with 5 px vertical padding. The conversation list is 240 px wide (previously 224 px), allowing the current selected title to fit on one line with 13 px text. Full model metadata remains available via its title tooltip when truncated.

## Verification

- Full UI suite: 805 passed, 0 failed, 1 skipped (806 total).
- A final regression run covers Agent, Home, shared controls and Settings after the control-height and tooltip adjustments: 130 passed.
- Read-only preview uses the existing selected conversation (39 entries and 14 saved conversation summaries). It does not send model requests or execute transfers. The preview omits native internal workflow state and credential references, and strips source URL query strings from the local job metadata. Its data, JSX and HTML are temporary and are removed before building the production frontend.
- Browser inspection at 1280 × 720: conversation 15/25/400; history titles 13/20/500; history metadata 12/18/400; panel heading 14/20/600. History width 240 px; the first title is one 20 px line. No page horizontal overflow.
- Light-theme preview: smallest visible interface text 12 px. An 840 × 720 viewport retained the existing responsive history collapse and a fully visible 488 px composer; no page horizontal overflow.
- Settings: page title 20/28/600, section title 14/20/600, labels 13/20 and help 12/18.
- Explore: section heading 14/20, source selector and input 13/20 at 32 px height; input padding 5 px vertically. No horizontal overflow.
- Screenshots: `.verification/typography-20261016/agent-dark.png`, `agent-light.png`, `agent-light-840.png`, `settings-light.png`, `explore-light.png`.

This verifies typography and interface behavior. Historical conversation content is preserved as historical content; no provider download or model workflow is revalidated by this preview. Native visual acceptance remains for the user in the rebuilt desktop.

Production frontend and native `custom-protocol` builds passed. Zero active jobs and zero active Agent sessions were confirmed before replacing the idle desktop. The rebuilt GeoD Global desktop was launched and its window responded normally. Temporary preview tabs were closed, their history copies removed, and the browser viewport restored.
