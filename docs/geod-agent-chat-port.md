# GeoD Agent chat presentation in Global

Global uses a local, versioned adaptation of GeoD Agent **0.2.5**, commit `693d6a5e2ee38b537543f173efa63e2914b1a0cf`. The reference checkout was read only. Five committed source files are captured with individual SHA-256 hashes in [the upstream inventory](../third-party/geod-agent-chat/UPSTREAM.json); the [MIT notice](../licenses/GeoD-Agent-Chat-MIT.txt) is retained. No sibling checkout or runtime dependency is introduced.

## Presentation synchronized on 2026-10-15

- Adjacent native read operations form compact resource-check groups. Other operations form tool groups. Commentary and native action cards retain chronological positions between groups.
- Active work remains in the transcript, with one current activity indicator. Model writing suppresses a separate waiting indicator. Native background work keeps the request active after an individual model reply ends.
- On completion, untouched process details collapse. Opening a group or tool detail pins that process open, so completion does not close content being inspected.
- Human messages always stay outside the process. The final reply is separate and has no repeated assistant name above it. Background continuation belongs to the same request rather than creating multiple final answers.
- Tool records keep bounded input/result receipts, durations and failure indications. User confirmation and unfinished native result cards stay actionable when the process is folded.

The adapted presentation lives in `prototype/src/ui/agent-chat.jsx` and `agent-chat.css`. `agent-chat-work.js` projects Global entries into the source component's activity groups while preserving Global's request lifecycle. The existing `message-text.jsx`, `message-scroller.jsx`, composer and shared UI foundation remain in use. Current upstream message/scroller primitives are captured for comparison; this update does not add their optional navigation rail.

Global retains its own model connection, source adapters, approvals, plan hashes, attachments, outputs and execution engine. Home conversations and Explore sidebar conversations preserve separate scene targets and drafts. Left-side sessions and right-side deliverables remain native Global panels. Model Markdown cannot create actionable file links, remote images or executable HTML; display receipts are not execution authority.

## Verification

[Current verification evidence](../prototype/qa/geod-agent-chat-port-20261015.json) records targeted regression tests, builds and browser rendering checks. Browser checks use an explicitly labelled isolated component replay, with no model call or download. They cover wide conversation, narrow sidebar, active/completed/interrupted states and inspection retained through completion. They are not evidence of a new end-to-end model or imagery-download run.

[Earlier port evidence](../prototype/qa/geod-agent-chat-port-verification.json) and [earlier lifecycle evidence](../prototype/qa/agent-turn-lifecycle-verification.json) remain historical records for the previous 0.2.4 port and native failure-path work; their counts are not current-run test totals.
