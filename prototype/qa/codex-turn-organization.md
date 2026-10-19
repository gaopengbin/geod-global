# Conversation activity by user request

Verified on 2026-10-10. This supersedes the earlier grouping by native model turn: a download request may continue through several model turns while downloads and processing run in the background. A native `final` message can be a stage update rather than the delivery summary.

## Organization

- Every human message starts a separate request block and remains outside its process disclosure. Greeting replies remain visible.
- Collect commentary, tools, stage replies and automatic workflow continuations in one chronological process for that request. Keep only the last current reply outside; while background work continues, its submitted-task reply stays in the process.
- Keep the process active through background waiting and automatic continuations, including intervals when the model is idle. Collapse automatically after execution finishes; preserve manual folding across continuations through a stable user-request identity.
- Native activity still owns streaming indicators and tool animations. A background workflow must not animate a historic tool or duplicate the composer status.
- Keep pending choices, pending reviews and unfinished task cards usable while folded. This also covers reviews referenced by the current reply. Expanding restores the original record order.
- Reveal completed result tasks from the right sidebar by expanding the corresponding request process.
- Use native workflow, goal and job receipts to establish lifecycle. An unrelated previously completed goal must not finish a new request. Text or `phase: final` alone does not establish file completion.
- Use a record count when no duration is available; never invent elapsed time.

The implementation uses Global's existing shared UI. Open-source [assistant-ui message parts](https://www.assistant-ui.com/docs/primitives/message) and [standalone tool UI](https://github.com/assistant-ui/assistant-ui/blob/main/apps/docs/content/docs/tools/tool-ui.mdx) were inspected as organizational references. No assistant-ui dependency or source asset was added. Earlier Codex bundle inspection was also a behavior reference only.

## Verification

- All 85 Agent panel integration tests passed, including download → mosaic → verification continuations, idle background work, manual folding, visible choices/reviews and separate human requests.
- All 16 grouping and verified-delivery unit tests passed, including reply-owned reviews and unrelated goal completion.
- Read-only replay of the selected real Shanghai history preserved all 35 original entries in exact order. Its 3 native workflow turns now form 1 request process; 2 submitted-task replies are inside it. The greeting and 1 delivery summary remain outside.
- The replay used the actual production AgentPanel and existing saved history with a visible read-only label. It made no model calls or new downloads and did not rewrite session data.
- Browser inspection verified the expanded chronology and collapsed result at desktop width and 390 px. The narrow panel had clientWidth and scrollWidth of 390 px.
- Repository isolation and shared UI boundary checks passed. Web and desktop builds passed.
- Refreshed only the owned local development desktop after checking that Agent turns and download jobs were idle. History and model settings were retained. No installer or production release was made.

The ignored `.verification/logical-request-turns` folder contains history validation, browser screenshots and local build/restart receipts. These demonstrate rendering of existing history, not a new model run or download.
