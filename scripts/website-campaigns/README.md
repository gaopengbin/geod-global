# Website campaign source

This is the readable first-party source of the website's bounded Landsat
campaign tracker and its privacy/receipt tests. It is covered by the repository's
GPL-3.0-only license. The application does not import another checkout at runtime.

The website workspace owns its copied asset and feature build step. Copy changes
as a reviewed artifact, compare the SHA-256 against `manifest.json`, rebuild the
website, and verify the public response. Do not create an absolute-path import,
workspace symlink or runtime dependency on this repository.

Website views/clicks and desktop installations have separate random identifiers.
Only the fixed public source/campaign labels are sent. `qa=acceptance-20261023`
marks the website's own acceptance events as `development`; it is not a user ID
or an access credential. Public promotion links omit this QA flag. DNT/GPC and
the persisted website opt-out prevent collection.

Run `npm run test:campaign` for browser-tracker, receiver HTTP and QA-excluded
reporting tests. Read production campaign observations with
`scripts/campaign-report.py` over authenticated server access; reports are not
public endpoints and should not be committed with internal identity registries.
