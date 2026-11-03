# CargoFrame

"CargoFrame" is a computer local context sharing application that send data in a user defined web destination.

A tray app for Windows 11 and Linux. A shortcut captures the program in focus, finds its project
folder, reads the data you asked for, and opens a window where you review it and share it to Jira,
Google, Reddit, Claude, Slack, Discord or any API you describe. Nothing leaves your computer until you click.

> **Status: V1 feature-complete and merged to `main`, in private beta.** Every feature in the
> plan is built and tested. What has **not** been done is running it on real desktops, with real
> accounts, and with a trusted signing certificate. See [`docs/status.md`](docs/status.md) for
> exactly what is and is not verified.

## What it does

1. You press your shortcut (each trigger has its own shortcut, capture mode and window layout).
2. CargoFrame captures the focused program (or the display), finds its folder, climbs to the
   folder with the program's name, and reads only the files your data rules allow.
3. A window opens with the blocks you designed: text boxes, dropdowns filled from the
   destination, the screenshot with crop, draw, shapes, text and colour tools, the files found
   with the ones you preselected, and a sharing button.
4. You review and press the button. If the share fails it is retried, then a zip is kept in your
   safe folder (or you are asked where to put it). Secrets never go in the zip. A button with no
   active connection says **Save locally** and saves that zip straight into the folder you set in
   Settings, General, Safety.
5. Layouts can start from a **ready-made template** for each connection (Jira bug, Jira comment,
   Drive upload, Sheet row, Reddit post, Claude prompt, Slack message, Discord message, Custom API, or a local save). A template is
   never changed; saving one makes your own copy. Layouts, data rules and info types can be
   **exported and imported** as one file (Settings, General, Export and import).

Data rules read JSON, YAML, TOML, INI, CSV, XML, a regular expression, or **plain text and log
files** (`.txt`, `.log`): a line, a range, the first or last lines, lines that contain a word, or
what follows a label.

Destinations in V1: **Jira** (the reference), **Google** (Drive, Sheets), **Reddit**,
**Claude** (hands the capture and your prompt to a chat; there is no chat inside CargoFrame) and
**Custom API** for anything else. Twitter/X, BlueSky and Instagram are planned for v1.1.

## Layout

| Path | Contents |
|------|----------|
| `app/` | The Tauri 2 program: tray, shortcuts, windows, commands, installer hooks |
| `ui/` | The Svelte 5 windows: Settings and CargoFrame (one page each) |
| `core/context` | Project-folder climb, `{{variable}}` templates, read-only allow-list |
| `core/safe` | Retry with backoff, failure classes, fallback ladder, zip naming |
| `core/layout` | Window layout model, grid, validation chips |
| `core/handoff` | Claude chat links, clipboard plan, pinned and recent chats |
| `core/config` | One JSON file per trigger, connection, rule, info type and layout; migration |
| `core/capture` | Screen and window capture (Windows, X11, Wayland portal), picture editing |
| `core/window` | Which program is in focus, and its path |
| `core/rules` | Data rules: read allowed files and pull out values |
| `core/bundle` | Zip files for the fallback and the Files block |
| `core/net` | The one HTTP client: https checks, checked redirects, multipart, mock server |
| `core/connectors` | Jira, Google, Reddit, Claude, Slack, Discord, Custom API, OAuth, the secret store |
| `docs/` | Decisions, status, security review, performance, signing, spike runbook |
| `scripts/` | CI helpers, end-to-end and timing checks, signing helpers |
| `installers/` | Windows (NSIS) and Linux (AppImage + install script) |
| `.github/` | CI, installer build, release (tag-driven draft) and dependency-check workflows |

## Develop

```
cd ui && npm ci && npm run build && cd ..   # the windows must be built once for the app
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cd ui && npx svelte-check && npx vitest run
```

**Windows: build the program and the installer.** Run `scripts\windows\build.cmd` (or double-click
it). It writes `target\release\cargoframe.exe` and
`target\release\bundle\nsis\CargoFrame_<version>_x64-setup.exe`. `build.cmd program` skips the
installer, `build.cmd install` builds and then opens the installer, and adding `fast` skips
reinstalling the web packages. In VS Code, **Ctrl+Shift+B** runs the same build (the tasks are in
`.vscode/tasks.json`, and Terminal, Run Task lists the others). It is a cmd script so that a
PowerShell "running scripts is disabled" setting cannot stop it.

Rust stable (see `rust-toolchain.toml`), Node 22. On Linux the app needs the WebKitGTK and GTK 3
development packages (the list is in `.github/workflows/ci.yml`). Run the program with
`cd app && ../ui/node_modules/.bin/tauri dev`. To try the windows alone in a browser with demo
data, run `npm run dev` in `ui/` and open `settings.html` or `capture.html`.

## Documents

- [Decisions](docs/decisions.md): what was decided, and how the project brief changed the plan
- [Status](docs/status.md): what works, what is verified, what is still open
- [Security](docs/security.md): the review, the findings and the known limits
- [Performance](docs/performance.md): budgets, how each is checked, what has been measured
- [Spikes](docs/spikes.md): the experiments to run on real machines
- [Signing](docs/signing.md): certificates and update keys (never commit these)
- [Installers](installers/README.md): what the Windows and Linux packages do
