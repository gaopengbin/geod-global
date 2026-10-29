# Signed Windows updates

Product: `xyz.laogao.geod.global`. Public configuration is frozen into `src-tauri/distribution/channel.json`. The updater channel is independent of GeoD Agent and the domestic application.

- Manifest: `https://geod-global.laogao.xyz/updates/windows-x86_64/latest.json` (website deployment, atomic replacement).
- Packages: immutable `https://download.laogao.xyz/global-updates/windows-x86_64/<version>/` objects; retain the matching GitHub release as an alternate.
- Signing key: DPAPI-protected file outside the repository at `%USERPROFILE%/.codex/secrets/geod-global-release/signing-key.dpapi`. Directory ACL permits the owner and SYSTEM. Never log, commit, or upload the private key. Back it up with the owner's normal encrypted credential backup; DPAPI ties this local copy to the Windows account/machine.
- `scripts/sign-global-release.ps1` decrypts only for the official pinned Tauri signing subprocess. Build commands explicitly strip signing secrets. `scripts/distribution-channel.py` requires a signature containing the exact release-version token.

The release must be completely tested before uploading and making its manifest current. Use the already accepted installer bytes; never rebuild after acceptance and silently substitute them.

## Delta policy

`scripts/build-update-delta.py` uses the existing publisher-side zstd CLI's prefix mode when available, with python-zstandard for an independent reconstruction check. A Python dictionary encoder is available as a fallback; its output still has to meet the savings threshold. These are publisher-only tools and are never installed by the application or this script. The previous immutable installer is the raw dictionary. The script verifies byte-for-byte reconstruction and records all hashes and encoder choice. A delta is offered only when smaller than 85% of the complete target. Native decoding uses pinned zstd 0.13.3, bounded base/target sizes, a bounded decode window and output count. The metadata records algorithm, URL, base/patch/target size and SHA-256. The rebuilt installer must also pass the target's version-bound Global signature; patch metadata alone never authorizes installation.

The installer retains one `installer-base.exe` in the application's local `distribution/` cache. This cache is not a project or download. A matching version and hash are required. Full installer and delta downloads follow the user's proxy preference. Cancellation stops the update attempt and never falls through to a full download. A failed delta automatically tries the original full URL, under the same explicit download confirmation. Downloads and reconstruction never edit live application files.

NSIS uses separate compression blocks so unchanged large runtime files remain reusable in future deltas. The historical 0.2.0 package used solid compression and shipped without the trusted channel, so it requires one manual bootstrap upgrade. Never claim that old installations will automatically receive this prompt.

## User flow and recovery

Checks are enabled by default, with the existing preference retained. Successful startup/focus checks use a six-hour interval. Transient network, timeout or upstream errors receive at most three later retries with minimum delays of one, five and fifteen minutes; permanent/signature errors are not repeatedly retried. A successful manual check resets retry state. A new version is announced once per app session; Later dismisses it for that session. The user can always check/download through Settings. Downloading and installing are separate confirmations. The existing lifecycle maintenance lock drains native commands and blocks installation while downloads, processing or the Agent are active.

The downloaded installer is rehashed and its signature/version rechecked just before launch. The installer receives the current executable's directory (including portable/custom paths) and waits for the updating process to exit before replacing files. The installer retains projects, credentials, histories, models and downloaded files in the application-local store. Keep the prior full installer for manual recovery; do not describe this NSIS installation as an atomic filesystem transaction or automatic rollback.

The ignored `distribution::tests::release_update_acceptance_probe` may be run only with an explicit configuration targeting `.verification/release-021/installed`. It uses the actual updater SDK and, when requested, launches the actual installer. Ordinary tests never install anything. Acceptance scripts must back up and restore the current-user uninstall registration, shortcut and updater base cache before using that isolated directory, and compare real user-data hashes before/after.
