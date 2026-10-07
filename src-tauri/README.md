# GeoD Global desktop shell

The independent Tauri 2 application embeds the approved workspace build and uses
`geod-runtime` directly. Desktop jobs do not require the browser companion HTTP
server. The application identifier is `xyz.laogao.geod.global`; job state lives in
the OS application-local data directory under this identifier, in `runtime/`.

The optional development Agent attaches to that same core through six scoped
commands. Prepare its owned Windows runtime with `npm run agent:prepare` before
`npm run desktop:dev`. Its session folder and secure model credential are separate
from personal Codex settings. See [Agent architecture and acceptance](../docs/agent.md)
for current read/search/plan capabilities, native confirmation, reproducible checks
and remaining release gates.

The main window uses typed application commands for health, local jobs, raster
inspection and executable recipes; it has no generic shell/filesystem command.
Raster inspection accepts a
completed SCL job identifier, validates its local file hash and decodes real pixels through
the same bounded Rust implementation as the development API. Reveal accepts a job identifier and reveals
only an existing successful output inside this application's storage directory.
Source links open in the system browser only after checking HTTPS and the exact
Earth Search, Sentinel COG, or AWS registry host. The same check applies to
navigation and new-window requests; remote pages never load inside the app.
Remote websites receive no IPC capabilities. The frontend does not receive
arbitrary filesystem, shell, or opener plugin permissions.

## Integrated desktop title bar and brand

The main window uses one 48px application title bar, containing the original
GeoD G mark, navigation collapse, project/page context and shared theme controls.
The actual Windows caption controls come from pinned
[`tauri-plugin-decoration` 3.0.5](https://github.com/oovz/tauri-plugin-decoration).
They retain native maximize/restore and Snap hit geometry. Closing still hides
to the existing tray. The title bar's explicit Exit app action explains task
retention and offers either hiding to the tray or fully shutting down workers.
The tray menu also retains its explicit exit action.

Windows system menus are opened through a command restricted to the caller's
window. Drag and double-click use only Tauri's `start_dragging` and
`internal_toggle_maximize` permissions. The plugin's closed readiness, geometry
and caption-action commands are scoped to the local `main` window. The capability
checker verifies all 32 application commands and this exact plugin allowlist;
there is no generic window, shell or filesystem permission.

Startup keeps native decorations enabled and the window hidden while activation
is awaited. Failure restores and reveals the native frame; an 8-second Rust
watchdog also reveals it if the frontend never becomes ready. The CSP adds only
the plugin's local stylesheet protocol, keeping remote navigation and IPC
restrictions intact. Browser development has no simulated caption controls.

App, executable, tray, About and favicon branding use the same original GeoD
symbol. Source provenance, original license and icon generation instructions are
in [`prototype/public/brand/README.md`](../prototype/public/brand/README.md).
The [appearance specification](../docs/design/desktop-appearance.md) describes
the design references and verification criteria.

From the repository root:

```sh
npm run build
cargo build -p geod-global-desktop --features custom-protocol
```

On Windows, this produces `target/debug/geod-global-desktop.exe`. It embeds
`prototype/dist` through the `custom-protocol` feature and can be started
without Vite. `tauri dev` uses the loopback Vite
development server configured at port 4317.

`npm run desktop:dev` is the primary development entry point. It owns Vite and
opens the native window with hot reload; do not start a separate Vite process on
the same port. Repeated launches restore and focus the existing main window
through the official [Single Instance plugin](https://v2.tauri.app/plugin/single-instance/)
before opening the exclusive runtime store. This adds no frontend filesystem,
shell, remote-origin or IPC permissions.

Closing the main window keeps the process in the native system tray; downloads
and raster workers continue independently of the webview's timers. Click the
tray icon to restore the window. Its localized menu can open the task page or
explicitly quit. Only explicit exit quiesces IPC, interrupts unfinished jobs and
waits for worker cleanup and saved state. Reopen to retry interrupted tasks from
the start. Completed files and user-cancelled statuses remain intact. The tray
uses [Tauri's Rust tray API](https://v2.tauri.app/learn/system-tray/), with no frontend tray control capability. Settings
explains this behavior only inside the desktop.

The task page can explicitly retry all failed/interrupted jobs through the same
existing retry command. Batch submission belongs to the application provider,
so navigating away from the task page does not lose the requested batch. It
never includes successful or user-cancelled jobs. Requests are serialized,
per-job actions are guarded, partial rejection remains visible, and disconnection
stops further submissions until the user reconnects and retries. This is a
restart from the beginning, not download byte resumption.

Read the [session acceptance record](../docs/releases/2026-10-01-desktop-session-acceptance.md)
for actual native-window evidence and the separate automated checks. Background
development does not require operating the user's visible desktop.

## Windows evaluation packages

From a source-frozen checkout, run:

```powershell
./scripts/package-windows.ps1 -Profile release -Installer nsis
```

This builds a fresh embedded frontend, desktop executable and CLI, then creates
an unsigned portable ZIP and optional current-user NSIS installer. It includes
examples, English workflow documentation, dependency licenses and source/hash
manifests. `-Installer none` needs no NSIS compiler. No system tool installation,
installer execution, signing or publication happens during packaging.

The installer removes only packaged files and empty directories. Application
data and extra user files remain. Tauri's built-in bundler stays disabled because
the repository packaging script owns this retention policy and the shared payload;
its NSIS intent is still declared in `tauri.conf.json`. See the complete
[Windows packaging guide](../docs/releases/windows-packaging.md).

The clean-source local evaluation ZIP and NSIS installer have now been built and
verified. See the [2026-09-22 artifact acceptance record](../docs/releases/2026-09-22-windows-artifact-acceptance.md)
for exact hashes, the packaged CLI workflow and desktop process-startup result.
Native UI/IPC interaction and install/uninstall acceptance remain separate checks.

## Earlier development startup evidence (2026-09-22)

- `cargo build --locked -p geod-global-desktop --features custom-protocol` passed
  after the multilingual frontend build, embedding `index-Bf6LlAmT.js`.
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

The historical language-switching/raster-inspection build was 21,541,888 bytes,
SHA-256 `8f99fdc7d2c1a0c06eba9e252c5d51390809ca8e42f406906cfc9c7c5aee6b41`.
Its hidden startup smoke check was responsive with empty stderr; the browser frontend plus
the shared Rust runtime passed real SCL inspection and offline/retry acceptance.
Detailed evidence is in [specification 10](../GeoD-Global-Spec/10-Languages-and-Raster-Inspection.md).
These historical hashes do not identify newer recipe or release builds. The
generated release manifest records the exact bytes of each new package.

Configuration and command permissions follow the official Tauri guides:
[configuration](https://v2.tauri.app/reference/config/) and
[capabilities](https://v2.tauri.app/security/capabilities/). External browser and
file explorer integration uses the [official opener plugin](https://v2.tauri.app/plugin/opener/).
