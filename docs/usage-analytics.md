# GeoD Global usage statistics

This implementation is independent of domestic GeoD, Agent hosted-model billing,
the public website, and WeChat Toolbox. The product key is always `geod-global`.
It introduces no third-party analytics SDK and requires no account.

## Default preferences and data boundary

New installations record and share anonymous usage by default, without signing
in or opening Settings. Settings clearly states the default and provides separate
switches for recording and sharing. A saved collection or sharing opt-out stays
off across restarts and upgrades; existing legacy off preferences are preserved.
The automatic default can use a receiver when one becomes available, but an
explicit sharing choice is bound to its configured address. An unreadable stored
preference fails closed rather than overriding a possible opt-out. DNT/GPC prevents
collection and sending. There is no retroactive import of conversations, project
history or completed downloads. `app_first_run` means the first recorded run,
not proof that an installer was downloaded or installed.

The event and field allowlists are in `schemas/usage-events.json`. Events contain
random event, installation, session and operation IDs, application version,
desktop/browser platform, development/production build environment, time, built-in source/category identifiers, outcome,
and coarse count/size/duration buckets. They contain no messages, model endpoint
or key, credentials, account identifiers, exact AOI or dates requested, imagery
IDs, project names, file names/paths, URLs, raw errors or device fingerprint.
Query strings from navigation are never recorded. A private/custom source is
represented by its protocol/category, not its name or URL.

Local records retain for 30 days and at most 2,000 events. The pending-job map,
bounded to 256 entries, contains local task IDs only to reconcile tasks after a
restart; it is **never uploaded or included in the exported report**. Turning
collection off clears events, pending jobs and the installation identifier and
aborts in-flight sending. Clearing local data rotates the identifier. It cannot
delete already received anonymous events; the receiver deletes these within 90
days with hourly maintenance. Reverse-proxy access logging must also be disabled
on the collection route if the operator wants the same no-access-log boundary.

## Events and outcome semantics

| Stage | Recorded events | Truth boundary |
| --- | --- | --- |
| Startup/navigation | `app_first_run`, `app_started`, `page_view` | Enabled statistics only; page names exclude IDs and parameters |
| Explore | `source_selected`, `scene_located`, `selection_changed` | Built-in source ID or generic custom-source category |
| Search | `search_started/completed/failed/cancelled`, `search_more_*` | Initial catalog search includes the NAIP latest-year probe; manual pagination is separate |
| Preview | `preview_requested/ready/failed` | Catalog imagery ready requires a rendered view after metadata and decoded tiles; public products require actual decoded nonempty tiles |
| Review | `download_review_opened`, `download_confirmed` | A confirmation is an action, not a transferred file |
| Queue | `download_submit_started/completed/failed/cancelled` | Accepted queue submission is not completion |
| Transfer | `download_completed/failed/cancelled` | A tracked native task reaches a terminal status, including reconciliation on the next open; `status` preserves succeeded/failed/cancelled/interrupted separately |
| Validation | `artifact_validated` | A succeeded task or a validated synchronous native result has an actual SHA-256 |
| Results | `project_saved`, `results_opened`, `artifact_opened` | Native save, explicit result action, or Workspace inspection succeeded; thumbnail requests are not result opens |
| Agent | `operation_*` with `agent_send/confirm/revise/stop`, `agent_turn_*` | Send acceptance is separate from the observed model-turn end; background download outcomes are independent |
| Other actions | `operation_*` with import/export/process/retry/connect | Records the validated client request result; dialog cancellation is separate |

Every event includes a `surface`/`channel` when applicable. `manual_explore`,
`agent_explore`, `agent_home`, `agent_workspace`, `workspace`, `library`, `tasks`
and `settings` remain separate. Agent turn tracking retains its originating
surface when polling on another page. Job tracking retains its originating
surface even if the user later opens Tasks or Workspace.

Request wrappers cover catalog products, public products, GSCloud originals,
custom STAC/WCS, feature services, official raster/vector interfaces, WMS, local
tiles/rasters/vectors, GEBCO, Overture, Earth Engine links, processing and model
connection/actions. Unsupported/poll-only operations produce no action events.
The normal runtime job poll observes terminal statuses; historical jobs that
were never tracked while statistics were enabled are not counted.

Cached native results can deliver a verified artifact without transferring new
bytes. File-size buckets are not network traffic measurements. Model request
acceptance is not a final Agent answer. Counts of reviews, requests, scenes,
files and projects have different units; do not divide these counts into a
purported task conversion rate. Random installation IDs are not people and
must not be added to website visitors, account totals or other-product IDs.

This change instruments usage, not an FPS/VRAM benchmark. Uninstrumented source
flows, closed-application model-turn ends, opted-out or privacy-blocked users, and failed
local storage must not be reported as measured zero usage.

## Queue and independent receiver

Anonymous sending flushes at startup, then uses batches of at most 50 every 15 seconds, with a 5-second
timeout and exponential retry up to 5 minutes. Stable event IDs survive retries
and the receiver deduplicates them. The client acknowledges only an exact
receipt for the sent batch; stale completion after revoking consent cannot
restore cleared records. Statistics failures never replace application results.
Requests omit cookies and referrers. Going offline retains a bounded queue;
coming online or hiding/leaving the page attempts a keepalive flush.

The Python standard-library receiver stores events in a dedicated SQLite DB:

```powershell
python -X utf8 services/usage/server.py --db .verification/usage.sqlite --port 4392
python -X utf8 services/usage/server.py --db .verification/usage.sqlite --report
```

The service binds only to loopback. `POST /v1/events` rejects extra fields,
unknown products/events, nonrandom IDs, free text categories, oversized batches,
unexpected browser origins and timestamps outside the supported window. Each
batch validates atomically. Access logging is disabled; transient in-memory
rate limits use client IP without storing it. `/v1/report` requires the operator
secret `GEOD_USAGE_ADMIN_TOKEN`; it is never shipped to the app. Browser origins
can be configured with `GEOD_USAGE_ORIGINS`. A CLI report against an absent DB
returns `not_collected`, not an invented zero-user report.

Reports group events by UTC date, source, surface, build environment, action/result and duration,
and list failure categories. They are backend summaries, not a public analytics
dashboard or proof that production has received events.

## Deployed receiver and application boundary

The independent receiver was deployed on 2026-10-19 as release
`20261019T092726Z`. It binds to `127.0.0.1:9119` under systemd service
`geod-global-usage`; its dedicated database is
`/srv/laogao/data/geod-global-usage/events.sqlite`. It shares no account or
analytics database with the website or other GeoD products. The uploaded archive
SHA-256 was `d593f1da2cabfce499912f935e5f5109ab81dfbbab96696f34c5006df99555a8`.
The previous Nginx configuration and deployment manifest are retained at
`/srv/laogao/backups/geod-global-usage/20261019T092726Z`.

Vite application builds and development previews now use
`https://geod-global.laogao.xyz/api/usage/v1/events` by default. Unit/UI tests do
not select this remote fallback. `VITE_USAGE_ENDPOINT` can override it before
building; an explicitly empty value creates a local-only build. Nginx exposes
only the exact events and health routes, disables their access logs, limits
bodies to 64 KiB and applies a separate per-IP request limit. Report URLs return
404 externally; operators read reports through authenticated server access.
The receiver trusts forwarded rate-limit addresses only from loopback, keeps
these addresses in transient memory, and does not save them with events.

The desktop CSP allows that specific origin; development additionally allows
loopback port 4392. A different origin requires a reviewed endpoint/CSP update
and a rebuilt desktop application. No wildcard network permission is added.

`scripts/verify-usage-production.mjs` verified the public HTTPS/CORS path,
default-on sending without a Settings call, exact receipts, rejection of a
sensitive field and opt-out. Independent readback from the production SQLite
database found exactly three unique canary events after a duplicate POST. All
canary events are marked `development`, not production user activity. Evidence
is in `.verification/usage-20261019/production-receiver.json`.

This verifies the live receiver and current client code, not a packaged WebView
or a new installer. The existing frozen installer is not rebuilt by this change.
Before release, rebuild the desktop application and verify one event from that
package reaches the receiver. Development and production environments remain
separate report filters.

## Packaged 0.2.0 acceptance, October 19

The Windows 0.2.0 installer and portable archive now passed actual first-launch
verification using separate fresh WebView profiles. Both default switches were
already enabled; no Settings toggle was changed. Production receiver readback
verified first-run, start and navigation events. A live installed-app search,
multi-selection, preview, download review, two fresh completed downloads, two
file validations and a local artifact open were also persisted.

See [the exact acceptance scope](releases/0.2.0-acceptance.md). The two test
installation IDs are recorded privately at
`/srv/laogao/data/geod-global-usage/qa-0.2.0-20261019.json`; exclude them when
querying real-user adoption. The raw receiver report intentionally retains QA
evidence and **does not automatically apply this external operator registry**.
Do not count these production-mode test installations as customers or people.
