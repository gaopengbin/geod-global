# Independent Global desktop account

Status (2026-10-08): independent Global email sign-in plus Google/GitHub are **deployed** at `https://geod-global.laogao.xyz`, release `20261007T162518Z`. Real desktop IPC verifies all three providers are available. Successful first email verification creates an account. Real delivery, human sign-in and restored desktop sessions still require user-driven acceptance. Development tests sent no real email.

The website account service remains in `G:/code/geod-global-launch`. Desktop production requests use `https://geod-global.laogao.xyz` through Rust. OAuth client secrets and provider tokens stay on the website server. Global accounts are separate from domestic GeoD accounts and NASA Earthdata/Copernicus data authorization. Local files, jobs and conversations remain local; signing in does not enable cloud synchronization.

## User flow

Open the account entry at the bottom of the navigation bar. “GeoD Global account sign-in” opens email verification in the system browser; Google/GitHub remain alternatives. First verified email or provider sign-in creates an independent Global account. The browser explicitly asks permission to sign in to the desktop app, including when it already has a website session. The native app verifies the returned code and identity before showing signed in and returning to the workspace. Guest use stays available while account service is slow or unavailable.

Email codes expire within five minutes and the desktop transaction deadline. The server enforces five verification attempts, one successful consumption, a 60-second recipient cooldown and ten daily sends per recipient, plus separate IP limits. Codes and recipient limit keys use HMAC with an independent Global pepper. Names and user IDs remain stable across email sign-ins. Matching emails do not automatically merge provider accounts. Sending acceptance, uncertain timeout, rejection and verified identity are separate; uncertain sends are never automatically retried.

The human explicitly authorized existing SES credentials/template reuse. Only necessary SES values were injected on the server into Global's restricted `email.json`, with a new pepper. Toolbox account/session/code tables and its full environment are not shared. SES uses the transactional channel (`TriggerType:1`) and GeoD Global sender display name/subject. Capability availability is not delivery evidence. Current credentials cannot read template/identity status; template suitability was confirmed by the human. This release did not change or independently attest historical credential rotation.

“Keep me signed in” stores only the independent Global session in Windows Credential Manager, namespaced by product and service origin. Without it, the session stays in native memory. Neither the renderer nor localStorage receives the token, PKCE verifier, OAuth secrets or browser cookies. Profile avatars are authenticated by Rust and passed as bounded image data, with an initials fallback.

Desktop login waits at most five minutes and can be cancelled. Cancellation invalidates the pending native generation; a late successful exchange is revoked instead of restoring login. Expired sessions require sign-in again. Local sign-out removes the saved credential and revokes the desktop session; network failure reports that remote revocation could not be confirmed. Website account deletion cascades to desktop sessions. Website and desktop sign-out are independent.

## Protocol v1

`product` is `xyz.laogao.geod.global`; protocol envelopes have `version:1`.

| Method | Path | Purpose |
| --- | --- | --- |
| GET | `/api/desktop-auth/providers` | Availability and `sessionTtlSeconds` |
| POST | `/api/desktop-auth/transactions` | `{provider,locale,redirectUri,state,codeChallenge,codeChallengeMethod:"S256"}`; returns `{product,version,transactionId,authorizeUrl,expiresAt}` |
| GET | `/api/desktop-auth/authorize?transaction=…` | Browser-bound consent; existing provider OAuth stays on the website |
| POST | `/api/desktop-auth/email/code` | Browser-bound same-origin form `{transaction,csrf,email}` |
| POST | `/api/desktop-auth/email/verify` | Browser-bound `{transaction,csrf,code}`; verified email creates a web session before desktop consent |
| POST | `/api/desktop-auth/confirm` | Same-origin bounded form, transaction CSRF and website-session CSRF; explicit approve or deny |
| POST | `/api/desktop-auth/exchange` | `{transactionId,code,verifier,redirectUri}`; atomic single use, returns `{product,version,accessToken,expiresAt,user}` |
| GET | `/api/desktop-auth/me` | Native Bearer session, returns `{product,version,user}`; 401 on expiry |
| GET | `/api/desktop-auth/avatar` | Native Bearer session; server-owned provider avatar proxy, no supplied URL |
| POST | `/api/desktop-auth/logout` | Native Bearer session revocation |

All timestamps are Unix seconds. User fields are `id`, `provider`, `name`, nullable `email`, boolean `emailVerified`, nullable `avatar`, and `expiresAt`. The server avatar value is only `/api/desktop-auth/avatar`; the public native snapshot replaces it with bounded image data. A missing GitHub email does not prevent sign-in. Identities are keyed by provider and subject; matching emails do not automatically link providers.

Provider IDs are `email`, `google`, `github`; native validation also accepts legacy two-provider responses. An email identity requires a verified, non-empty email. Desktop browser pages use `Referrer-Policy: strict-origin`: Chromium supplies a valid same-origin form POST Origin without leaking transaction query strings. Missing/null/cross-origin form Origins remain rejected. Real-browser testing caught the prior `no-referrer` / `Origin:null` rejection that explicit-header HTTP fixtures missed.

Email acceptance evidence: 35 backend tests passed locally and on the Linux candidate using synthetic accounts/mail; 8 native identity tests and 26 relevant UI tests passed. Six layout cases and the hosted form at 960/360px were inspected. The production email form, same-browser cancel and loopback state passed without sending mail. Native screenshot: `prototype/qa/identity-native-zh-email.png`. The updated development executable runs from `.verification/goals-desktop-target/debug`; no installer was built.

Transactions expire after 300 seconds; approved codes after 60 seconds; desktop sessions after seven days. No refresh endpoint exists. Callback addresses are restricted to `http://127.0.0.1:<ephemeral-port>/auth/callback`, port 1024–65535, without user info, extra path, query or fragment. Native state and S256 PKCE protect this handoff independently of the website-to-provider OAuth flow. The callback contains only `code/state` or `error=access_denied/state`. Native requests have bounded bodies/timeouts and do not follow server redirects. Session/code values are stored as hashes on the server.

The external-browser, loopback and PKCE design follows [RFC 8252](https://www.rfc-editor.org/rfc/rfc8252) and [RFC 7636](https://www.rfc-editor.org/rfc/rfc7636).

## Validation and release boundary

- Website: original 11 signed synthetic OAuth/account tests and nine desktop transaction/consent/PKCE/replay/expiry/delete/rate-limit tests. No real provider authorization or email is triggered by these tests.
- Native: seven tests using real local HTTP sockets cover callback policy, S256 exchange, safe snapshot, secure-storage abstraction, restore, session-only mode, cancellation races, storage failures and revocation.
- UI: eight integration tests cover sign-in, pending/cancel, guest use during slow checks, unavailable-server retry, rejected secret/error payloads, profile/avatar fallback, sign-out and Chinese copy.
- Visual checks render actual App and SignInPage in English/Chinese, light/dark, desktop and narrow layouts. Controlled identity snapshots validate layout, not real OAuth acceptance.

Deployment must include `tools/desktop_auth.py`, the updated `tools/auth_app.py`, `site/desktop-auth.css` and `site/desktop-auth.js`. `tools/prepare_release.py` includes the new server module and tests. Existing OAuth callback URLs/configuration stay in use; no client secret needs copying into the desktop repository. Back up the existing SQLite database before the server migration, preserve users/sessions/waitlist, and perform user-driven real Google/GitHub desktop acceptance after deployment. An unconfigured or older account server must remain an honest unavailable state.

The 2026-10-07 account-only release preserves the prior website assets and OAuth configuration. All 24 website/account/desktop/waitlist tests passed both locally and against the staged Linux payload. Live checks cover provider availability, anonymous transaction creation, browser-bound cancellation and denied-code rejection; no real provider authorization was initiated. Database snapshots and integrity checks passed, and existing identities remained intact. Private rollback snapshots are at `/srv/laogao/backups/geod-global/deployment-20261007T154805Z`. Full release and rollback details are in the website project's `AUTH-DESKTOP-IMPLEMENTATION.md`. The existing website still lacks `/terms.html`; this release does not invent or publish legal terms.

Debug builds can use `GEOD_GLOBAL_DEV_AUTH_ORIGIN=http://127.0.0.1:<port>` for a local account server. Release builds always use the public origin. This does not enable fake provider identities in normal application code.
