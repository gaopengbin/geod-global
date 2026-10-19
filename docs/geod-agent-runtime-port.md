# Agent tool context and request usage

2026-10-15. Source reference: GeoD Agent 0.2.5, revision `693d6a5e2ee38b537543f173efa63e2914b1a0cf`. The three original source files, hashes and MIT license are captured in [the source manifest](../third-party/geod-agent-runtime/UPSTREAM.json). Global has no runtime dependency on that checkout.

## What changed

- Tool schemas are selected for the current human request. Closed calls from older tasks no longer activate every earlier tool family. Known task families are immediately available, avoiding an initial discovery round trip; missing families remain reachable through the existing native loader. Explicitly loaded groups survive the current turn's context organization. Unresolved tool pairs and native workflow tools remain available.
- Strict greetings and thanks use a small no-tool prompt, as in the reference implementation. This requires the native human request to match the current model input, no attachments, no pending tool calls, and no context compaction or workflow continuation. The original transcript is never rewritten. Task requests and internal compaction retain full business instructions and original history.
- Stable native reads continue to reuse their existing per-turn receipts. Native background observation still waits for settled jobs without repeatedly calling the model. This change does not add parallel execution or remove confirmation and result verification.
- Every displayed human round has one usage button at its final answer or current work group. It shows actual input, output, cached input, reasoning output and provider-request counts. Background continuation and compaction belong to that request. Cache and reasoning are subsets, not extra tokens.
- Provider attempts that fail without a receipt remain unknown. A late receipt uses the owner captured when generation started, even after a stop or a new turn. Local requests rejected before generation do not count as provider attempts. Old or truncated records show partial/unavailable usage rather than invented zeroes.
- Session list snapshots no longer duplicate each conversation's detailed usage ledger; only the selected conversation carries its bounded ledger.

## Actual model check

The isolated native test host reused the saved `deepseek-flash` connection through the existing credential vault. No credentials were created or copied into the repository, and the user's registry was verified unchanged. Each request used the real bundled Agent and native tool dispatcher. The isolated workspace intentionally contained no saved projects.

| Request | Model requests | Input tokens | Output tokens | Cached input | Actual tools |
| --- | ---: | ---: | ---: | ---: | --- |
| Greeting | 1 | 124 | 74 | 0 | None |
| Read saved projects | 2 | 19,560 | 159 | 11,648 | `geod_projects_list` |
| Thanks after tool use | 1 | 120 | 77 | 0 | None |

Before adding the conversation-only prompt in this same run, the same initial greeting consumed 6,561 input tokens. The final initial greeting consumed 124: approximately 98.1% less input for this specific greeting. This is a measured prompt reduction, not a billing reduction or an estimate for download tasks. Both final greetings reported zero cached input.

The project query exposed 17 of 71 registered tools on each model request, with 10,772 characters of tool definitions versus 59,666 for the complete registered set. The earlier Global implementation already deferred groups, so this full-set comparison is not presented as a before/after token saving. The query completed without a separate `geod_tools_load` call. It reported the actual empty test workspace and created no files or download jobs.

## Validation and boundaries

- 153 Agent/runtime tests passed, including failed requests, native compaction, scene isolation, late receipts, exact tool schemas and original approval boundaries.
- 88 AgentPanel integration tests passed, including the usage footer and keyboard dismissal.
- 45 related frontend tests passed, including accounting, bounded snapshot validation, per-request transcript grouping and legacy records.
- The ignored native live model harness passed. Three actual requests were checked. Provider calls totalled four.
- Browser checks used the actual AgentPanel and production CSS to replay captured live receipts in the conversation layout and a 420px sidebar. Both displayed matching totals; no browser console errors were observed. This is renderer verification, not a claim of native window inspection.
- Development desktop build and dependency-isolation checks passed. No installer, publication, or user-session restart was performed. The captured report is [here](../prototype/qa/geod-agent-runtime-20261015.json).

General history archiving/retrieval, the entire upstream scheduling engine, and provider price calculations are not part of this port. A fresh full satellite download or multi-source workflow benchmark was not run in this change; its overall savings remain unmeasured. Existing context-window configuration, native compaction and result validation remain in place.
