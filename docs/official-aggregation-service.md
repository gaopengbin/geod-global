# Official aggregation service — local beta implementation

Date: 2026-10-09. This is executable server/client code, not a deployed commercial offer. Live collection, automatic renewal, managed model inference and cloud raster processing remain disabled/unimplemented. Test credits and test prices are explicitly labelled in the UI.

## Boundaries

- The existing GPL desktop, BYOK Agent, direct public-source connections and local processing remain available. No local file is locked by membership or balance.
- The optional service runs in a separate process. It actually schedules public catalog searches, persists results, deduplicates alerts and meters completed checks. A modified client flag does not change the official server's authority.
- It validates every private request against the existing Global desktop identity authority. It uses the stable user ID, never an email submitted by the client. Revoked sessions are not cached.
- The first service is multi-area imagery monitoring. One monitor selects Earth Search Sentinel-2 L2A or Planetary Computer Landsat. Both upstreams were queried live during local verification. Only these two sources are admitted to this beta service; the wider desktop source directory is not advertised as server coverage.
- A query match does not prove complete imagery coverage or regional cloud quality. Scene clouds remain scene averages. No monitor purchases imagery or starts a download. Previewing a notification opens the existing search/map flow and subsequent native task confirmation.
- Code in this repository retains the existing license. Self-hosting is possible; it grants no access to an independently operated official instance.

## Server setup

Requires Python 3.12. Install `services/official/requirements.txt` in the server's own virtual environment. Start from this repository root:

```powershell
python -m services.official.app
```

It listens on `127.0.0.1:4319`, stores state in `.geod-service/official.sqlite3`, and disables access logging. Configure `GEOD_SERVICE_DB` with a persistent private server path, `GEOD_SERVICE_PORT` for the listener and `GEOD_ACCOUNT_ORIGIN` for the fixed Global identity service. HTTPS is required outside local loopback. No sibling checkout is imported.

The native client uses the account origin's `/api/services/v1/` prefix. A future approved deployment should reverse-proxy only this prefix to the service. Keep the identity routes with the existing account service. Use TLS, request/body/rate limits, private SQLite directory permissions, backups and a single supervised worker instance. No deployment occurred in this implementation round.

Debug desktop builds may set `GEOD_GLOBAL_DEV_SERVICE_ORIGIN=http://127.0.0.1:4319`. The production build cannot override this origin from environment or the webview. The desktop token stays in Rust/OS secure storage; it is never passed to React, the model process or an upstream imagery catalog.

## Beta access and authority

Pro is a server subscription row. There is deliberately no HTTP endpoint to grant Pro, set balances, approve refunds or inject arbitrary consumption. A local server operator can grant an expiring test entitlement with an audit reason:

```powershell
python -m services.official.manage --db <private-database> grant-beta-pro --user <Global-user-id> --days 30 --operator <operator> --reason <reason>
python -m services.official.manage --db <private-database> revoke-pro --user <Global-user-id> --operator <operator> --reason <reason>
```

This is an operator command on a controlled server, not a secret client activation code. A user deploying their own service can administer their own instance.

The beta admits at most 10 areas per user, 2,000 boundary vertices per area and 30 degrees of extent in either direction. Dateline areas and larger scopes are rejected explicitly. A check follows at most ten pages of 100 items; partial results are labelled and not charged. The selected polygon/holes are passed upstream unchanged. Only a same-provider cursor is reused; pagination cannot redirect to a private or foreign host or replace the approved filters.

## Review, execution and ledger

1. Signed-in Pro prepares an immutable 15-minute quote containing polygon, source, lookback, cloud filter, interval, policy version, per-check test cost and daily/total ceilings.
2. The user reviews the complete monitoring card. Acceptance checks owner, hash, expiry and current server Pro before saving one monitor. Repeated acceptance returns the same monitor.
3. The worker enqueues due checks in SQLite. Each logical check has an idempotency key and cannot overlap another active check for that area.
4. The server atomically checks daily/total budget and reserves credits from expiring grants first, then purchased credits. Reservations count towards the authorized ceilings.
5. Successful, exhausted catalog queries settle one test credit. Failed, partial or interrupted checks release unused reservations to their original buckets. An unknown interrupted public-catalog query is absorbed by the platform, not estimated and charged.
6. New item IDs create one notification per check. Subsequent identical results do not repeat the alert. Original result counts and partial/complete flags remain distinct from abbreviated UI item lists.

Schema version 2 uses WAL transactions, grant invariants and unique business keys. Purchased credits have no expiry in this beta. Annual terms grant 300 test credits at each monthly calendar anchor, never 3,600 up front. Monthly grants expire at the next anchor; no local artifacts are deleted on downgrade. The first validated identity receives one 100-credit test grant; reinstalling does not grant another.

## Desktop and Agent

The sidebar's **Imagery monitoring** page contains area/conditions forms, confirmation cards, monitored areas and bounded result lists. New imagery alerts also appear in the existing notification bell. Membership, credits, usage and orders live on **Your account**, opened from the signed-in avatar. Unavailable monitoring is labelled as preparation with working Agent/search actions. Sandbox checkout controls appear only to explicitly invited test customers; ordinary accounts do not see a payment test console.

Agent tools are deliberately limited:

- `geod_official_services`: read real capability and entitlement.
- `geod_monitor_list`: read current server monitors/results.
- `geod_monitor_notifications`: read deduplicated alerts.
- `geod_monitor_quote`: prepare a review using a verified native administrative-boundary reference or the explicitly selected polygon. Arbitrary model vertices are rejected; original coordinates stay out of model history.

The model cannot purchase, grant membership or accept monitoring through these tools. A native change signal refreshes server reviews; monitoring confirmation occurs in the current conversation's own complete card or in Imagery monitoring. Reviews are filtered by conversation for the chat display and remain account-owned on the server. The full-access mode for local download plans does not silently grant a server monitoring budget. New dynamic tools change the tool-set signature: older saved conversations retain their original safety binding and may require a new conversation; they are not migrated as if they used a different model.

## Payment adapter

The adapter supports **Stripe test mode only**, with `GEOD_STRIPE_TEST_KEY` and `GEOD_STRIPE_TEST_WEBHOOK_SECRET` in the server environment. It rejects live keys. No credentials were configured, no live checkout was opened and no real Stripe transaction was performed during this round. Without configuration the UI and API cannot start checkout.

Sandbox checkout additionally requires a server-side invitation: `GEOD_SERVICE_TEST_CUSTOMERS` is a comma-separated list of explicitly approved Global user IDs. No browser/client flag can invite a user. Missing invitations disable test checkout; an authenticated, uninvited account is still denied when keys are configured. This prevents a public beta user from obtaining real server resources with freely available test payment credentials.

Products are the prior proposed USD amounts, used only as test fixtures. Pro is a prepaid month/year term with no automatic renewal. Starting checkout is a separate explicit review; it never approves a download. The server selects price/currency/SKU. A browser success redirect never grants entitlement.

Raw-body, time-bounded webhook signatures are verified. Events are saved before acknowledgement; the worker independently retrieves the checkout session and verifies order, session, mode, paid state, amount and currency. Fulfillment and ledger writes are one transaction. Duplicate events/payment IDs cannot grant again. Restore purchases retrieves the user's own pending sessions even when a webhook was lost.

The user can request refund review for their own fulfilled test order. A server operator can process an approved test refund:

```powershell
python -m services.official.manage --db <private-database> process-test-refund --request <refund-request-id> --amount-minor <USD-cents> --operator <operator> --reason <reason>
```

Unused purchased credits support a full/partial refund under the current one-cent-per-credit test prices. An entirely unused Pro term supports a full refund; used-term/prorated policies require later business approval. The account's new official work is frozen during provider reconciliation so withdrawal cannot race spending. Reserved or consumed purchased credits cannot be refunded as unused. Idempotent provider requests survive retries; successful provider receipts void only the original bucket or revoke that term, and failed receipts restore new-service access without removing credits. Provider success is recorded separately from bank settlement. Refund/dispute automation and recurring renewal are not claimed complete.

Primary adapter references: [Stripe checkout creation](https://docs.stripe.com/api/checkout/sessions/create), [webhook verification and retry guidance](https://docs.stripe.com/events/manage-webhook-endpoints), [refund API](https://docs.stripe.com/api/refunds/create).

## Verification and remaining gates

Run `npm run test:services` after installing the server requirements. It uses synthetic identities, public-provider fixtures and signed synthetic payment events; it does not send verification emails or charge cards. Existing UI/native tests separately check confirmation, account switching and the token-free bridge. CI runs the service tests on Linux and Windows.

Local regression results: 20 server tests, 380 frontend tests, 120 Agent tests and 55 native desktop tests passed. The 26 native tests requiring explicitly configured live models/credentials or separate human acceptance remain ignored. Repository checks, warnings-as-errors Clippy and the frontend build passed. Desktop-width and 390-pixel browser layouts were inspected; Pro forms used a visibly labelled synthetic UI fixture. No native-window customer acceptance is claimed. [Verification record](../prototype/qa/official-service-verification.json).

Local live public-catalog evidence is stored under ignored `.verification/official-service-live/report.json`: Manhattan test polygon, 15 Sentinel-2 matches and 10 Landsat matches in the queried 30-day interval. Identity is an explicitly synthetic isolated operator fixture; catalog responses are real. Checks completed, test reservations settled and alerts were stored. This is not a customer-login, cloud-deployment, download-coverage or payment-merchant receipt.

Before public service/sales: live Global login/native use acceptance; approved rollback-capable deployment; source-operation terms and actual cost review; real Stripe sandbox payment/refund receipts; merchant/country/tax/terms decisions; final prices; live payment adapter/reconciliation and renewal/dispute work if offered; human purchase-to-service-to-bill acceptance. Managed AI, additional hosted advanced-source automation and cloud raster processing remain separate unimplemented services, rather than being unlocked by this beta's Pro flag.
