# GeoD Global desktop shell

The independent Tauri 2 application embeds the approved workspace build and uses
`geod-runtime` directly. Desktop jobs do not require the browser companion HTTP
server. The application identifier is `xyz.laogao.geod.global`; job state lives in
the OS application-local data directory under this identifier, in `runtime/`.

The main window can invoke only `health`, `list_jobs`, `create_job`, `cancel_job`,
`retry_job`, `reveal_job`, and `open_source`. Reveal accepts a job identifier and reveals
only an existing successful output inside this application's storage directory.
Source links open in the system browser only after checking HTTPS and the exact
Earth Search, Sentinel COG, or AWS registry host. The same check applies to
navigation and new-window requests; remote pages never load inside the app.
Remote websites receive no IPC capabilities. The frontend does not receive
arbitrary filesystem, shell, or opener plugin permissions.

From the repository root:

```sh
npm run build
cargo build -p geod-global-desktop --features custom-protocol
```

On Windows, this produces `target/debug/geod-global-desktop.exe`. It embeds
`prototype/dist` through the `custom-protocol` feature and can be started
without Vite. `tauri dev` uses the loopback Vite
development server configured at port 4317.

This initial shell is a development build. Installer generation, signing,
updating, and macOS/Linux packaging are not enabled or validated here.

## Verification on Windows (2026-09-22)

- `cargo build --locked -p geod-global-desktop --features custom-protocol` passed
  after the final frontend build, embedding `index-BngLfz1L.js`.
- `cargo test -p geod-global-desktop --features custom-protocol` passed all three tests:
  navigation stays on the app origin, source URLs stay within the approved
  HTTPS domains, and Reveal accepts only an existing, successful output within
  the runtime storage directory.
- The resulting executable was launched with `Start-Process -WindowStyle Hidden`.
  Its process remained responsive, its error log was empty, and it created an
  empty `runtime/jobs.json` in its separate application-local data directory.
  The smoke-test process was stopped afterward.
- This is process and storage startup evidence. Native-window visual acceptance
  and a complete UI-to-IPC download/reveal interaction still require validation.

Configuration and command permissions follow the official Tauri guides:
[configuration](https://v2.tauri.app/reference/config/) and
[capabilities](https://v2.tauri.app/security/capabilities/). External browser and
file explorer integration uses the [official opener plugin](https://v2.tauri.app/plugin/opener/).
