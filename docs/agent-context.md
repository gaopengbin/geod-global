# Agent model context settings

Each saved model connection has an optional `contextWindow` integer in tokens.
An empty field means automatic. Explicit values from 8,192 through 4,194,304 are
validated in the desktop registry, Node bridge and frontend. They must fit the
actual model or gateway's limit; the setting does not increase provider capacity.

The documented DeepSeek Flash / V4 Pro routes on `https://api.deepseek.com`
(root or `/v1`) default to 1,048,576 tokens. Unknown models and gateway aliases
retain a conservative 32,768-token fallback and the form explains how to override
it. Presets were checked against [DeepSeek's model metadata](https://api-docs.deepseek.com/api/list-models/)
on 2026-10-09. Endpoint and model both matter; a gateway's alias alone is not proof
of its capacity. The source of this policy is `agent/model-context.mjs`.

The native Codex configuration receives the resolved capacity. Automatic
organization counts the **total** active context, including the instruction
prefix, at up to 85% of capacity, with output and additional headroom reserved.
The bridge's output budget is capped for small windows. Its former 2 MB text
guard scales with capacity, while the independent 32 MB request bound remains.
Context usage receivers support the same upper bound; they still show only
observed native usage, not estimated percentages.

Capacity changes close and recreate the idle runtime on the next operation.
Changing this setting alone keeps the connection identity, native thread,
conversation history, pending reviews and task goal. Model changes remain blocked
while a response is running. No API key needs to be reentered for a context-only
change. Empty/omitted settings remain compatible with existing registry files.

This setting is separate from per-response tool-call limits and upstream rate
limits. Increasing it does not bypass permissions or approve a download.

## Request overhead and usage accounting

As of 2026-10-11, new native threads begin with 11 core tool schemas instead of
the complete 67-tool catalog. `geod_tools_load` exposes named, pinned native
groups when needed. Native workflow continuations begin with 18 tools for
project/job verification and download/processing; they can discover additional
groups in the same way. This only changes model context: native permissions,
scene scope, review hashes, coverage checks and output verification still apply.
Existing native threads without the loader retain their original complete
contract. Start a new conversation to obtain the initial discovery reduction.

The shared application instructions are consolidated in `agent/instructions.mjs`.
History and provider reasoning/tool pairs remain intact. Automatic organization
still follows the configured context window, not an arbitrary low token cap.
Tool results and continued history can still make later requests substantially
larger than the first request; discovery is not a complete bound on task cost.

The composer context popover separates the latest native context occupancy from
**recorded cumulative model usage**. The provider bridge records input, output,
total, cached input and reasoning output in the active session. Cached input is
part of input, and reasoning is part of output: neither is added a second time.
DeepSeek's separate `prompt_cache_hit_tokens` field is retained. Unreported
details are unknown, not zero. These counters are not the provider's invoice.

Each session persists all-time totals since collection began and the most recent
128 request records, including human/workflow origin and tool-schema character
counts. The usage ledger does not contain request text, private reasoning or
credentials. It cannot reconstruct old requests or usage omitted by a provider,
including a disconnected stream that never delivered usage. Native goal-engine
`tokensUsed` is separate and is not used as a cumulative billing counter.

## Real-model regression evidence, 2026-10-11

Same saved `deepseek-flash` connection and original task prompts; June 2025
Sentinel-2 L2A SCL, native 20 m pixels, three native review confirmations per
scenario, actual downloads and local clipping. The verification driver confirms
only its own bounded SCL plans through the same native approval endpoint used
by the UI. Browser rendering was inspected separately; this run does not claim
native-window click coverage.

| Scenario | Baseline total tokens | Optimized total tokens | Reduction | Requests, before → after |
| --- | ---: | ---: | ---: | ---: |
| Explore sidebar, San Francisco rectangle | 615,865 | 308,799 | 49.86% | 17 → 15 |
| Home conversation, Manhattan administrative polygon | 716,212 | 502,453 | 29.85% | 18 → 20 |
| Combined | 1,332,077 | 811,252 | 39.10% | 35 → 35 |

Both goals and all declared outputs were verified. Download hashes were checked;
both clipped rasters have identical pixels, dimensions, CRS and transforms to
the baseline. Manhattan's complete TIFF hash is also unchanged. The SF task
chose the native mosaic path instead of the direct clip path, so its TIFF bytes
differ while raster pixels and georeferencing match exactly. The two sessions
retained independent scopes and file IDs.

First-request input was 8,573 / 8,591 tokens. Native continuations carried 18 tool
schemas. The final two sessions recorded 263,808 / 461,312 cached input tokens,
already included in the totals above. Baseline cache data was not collected, so
no cost-reduction percentage is asserted.

This is one final run per scenario, not a statistical performance guarantee.
The earlier full optimization pass consumed 320,183 / 736,379 tokens; it exposed
unnecessary science-tool discovery and excessive pagination, prompting the
second refinement. Manhattan still used more model requests than the baseline;
remaining serial discovery and accumulating history should be measured before
claiming that total task consumption is low.

Local evidence (ignored verification artifacts):

- Baseline: `.verification/live-scene-ui-1791646428030924800/execution-token-audit.json`
- Earlier pass: `.verification/live-scene-ui-1791700412132212000/optimization-report.json`
- Final: `.verification/live-scene-ui-1791701532544308000/optimization-comparison.json`
- Provider records and jobs: the final directory's `optimization-report.json`
- Browser: the final directory's `usage-panel.png`
