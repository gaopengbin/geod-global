# GeoD Global 0.2.1

This release improves map preview recovery and failure diagnostics, and opens the independent signed Windows update channel.

- Map previews retry temporary connection and upstream errors within a bounded deadline. A failed metadata request no longer leaves the map waiting indefinitely. Retry after restoring the local service is supported.
- Anonymous failure events now identify the operation stage, a fixed reason code, HTTP status when known and retry count. Raw errors, coordinates, credentials, local paths and source URLs are excluded. Historical events cannot be reconstructed retroactively.
- The desktop checks for new releases automatically and shows a compact update dialog. Download requires confirmation, and installation requires a separate restart confirmation. Active downloads, processing and Agent work must finish before installation.
- Updates require a Global-only, version-bound Minisign signature. A matching, sufficiently smaller zstd delta can reconstruct the complete installer; missing/stale base files, failed transfers or invalid reconstruction fall back to the full signed installer. The complete installer is verified again before installation.
- Custom installation locations are upgraded in place. Installer startup waits for the updating process to exit. Projects, model connections, downloaded files and preferences remain outside the installer payload.

## Upgrading from 0.2.0

**Install 0.2.1 manually once.** The public 0.2.0 installer did not embed a production update endpoint or trusted Global public key and therefore cannot receive this release's prompt. After installing this release, future versions can use the built-in updater. The installer caches one complete installation package to support future delta downloads. If that cache is unavailable, the full update remains supported.

Updater signatures authenticate release bytes; they are separate from Windows Authenticode. Windows may still show the existing unsigned-publisher warning. This release does not add or change data-provider entitlements, hosted-model login requirements or telemetry opt-out choices.
