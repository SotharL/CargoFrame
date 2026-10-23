# Decisions

The full plan lives in the Claude Docs document "CargoFrame – Development Plan (Windows 11 &
Linux)". This file keeps the decisions that shape the code, plus where the project brief changed
the plan.

## Product

- **Platforms (v1).** Windows 11 and Linux (GNOME and KDE, Wayland and X11), x64 only.
- **Triggers and layouts.** Several triggers, each with its own shortcut, capture mode and its
  own layout. Sharing one layout between triggers is a later option.
- **Fresh install is empty.** No connections, rules, triggers or layouts. An empty layout opens
  an empty window with a hint and an "Edit layout" button.
- **Nothing leaves without a click.** No telemetry, no automatic upload.
- **Run on sign-in** is on by default in the installer.

## Destinations

- **v1:** Jira (reference destination), Google (Drive, Sheets), Reddit, Claude, Custom API.
- **Claude is a one-way share target.** There is no chat inside CargoFrame and no API key. A share
  puts the screenshot on the clipboard and opens a chat by link. The dropdown lists pinned
  chats, then recent chats, then "New chat"; the person pastes a chat link once to pin it. The
  prompt is a text block with the **Prompt** role, bound to the sharing button; the button stays
  disabled until one is bound. An AI provider interface stays open for later.
- **v1.1:** Instagram (needs a public image URL and JPEG conversion), BlueSky and Twitter/X.

## Architecture

- One Rust core, Tauri 2, Svelte UI. The always-on process is tray-only with no webview; the
  Settings and CargoFrame windows are created on demand.
- Logic lives in small crates under `core/` that know nothing about Tauri or windows, so it is
  tested without a display. Side effects (files, the secret store, network, the clock) sit behind
  traits and are injected; tests use a local mock server and an in-memory secret store.
- Secrets live in the OS credential store, never in config files, logs or zips.
- Install paths: program in `%LocalAppData%\CargoFrame`, settings in `%AppData%\CargoFrame`
  (Linux: `~/.local/share/CargoFrame`, `~/.config/CargoFrame`). Local data (logs, kept zips) sits in a `data` subfolder of the program folder, so "delete my data" never touches the program.

## Memory, speed and security (Phase 5)

- **Memory modes** (Settings, General, Memory). *Low memory* destroys a window when it is closed.
  *Balanced* (default) hides it and destroys it after 5 minutes unless it was opened again.
  *Always warm* only ever hides it. The tray-only process never holds a web view. Measured on a
  virtual display: reopening a kept window 36 ms, a cold one 823 ms.
- **Performance budgets are gates, not hopes.** Each budget in `docs/performance.md` has a check
  in CI. A gate fails at budget +20%. Shared runners are slower, so their timing gates run at
  twice the budget (`CARGOFRAME_PERF_SCALE=2`). A budget changes only by changing the plan first.
- **Memory is judged on anonymous memory on Linux.** The resident set counts shared libraries
  that the toolkit maps for every program; the 40 MB budget still has to be confirmed on a
  reference machine (spike S6).
- **One HTTP client, redirects checked by us.** Every hop must be https (plain http only between
  this computer's own test servers), may not go from https to http, may not carry a user name,
  and may not take `Authorization`, `Proxy-Authorization` or `Cookie` to another site. At most 5
  hops. A Custom API connection follows none: a redirect is reported as an error, so its key
  can never be sent to a place the person did not name.
- **The CargoFrame window is the less trusted window.** It gets no settings, secrets or
  connection commands, and a share uses what the backend holds for that capture, not what the
  window sends. Tests fail if the content security policy or the permissions are loosened.
- **Timings stay local.** The program times its own shortcut-to-window path and shows the last
  numbers in Settings. They are plain milliseconds in memory and the log, never sent anywhere.
- **Releases are drafts.** Pushing a tag `vX.Y.Z` (the same as the version in `Cargo.toml`)
  builds both installers, runs their smoke tests, and creates a **draft** GitHub release with
  checksums. Nothing is published until the owner opens the draft and publishes it. The
  in-app updater stays off while the repository is private.

## Changes from the project brief

The Project description added requirements the first plan did not spell out. They are adopted
as follows.

| From the brief | How it is handled |
|----------------|-------------------|
| Capture the program in focus; fall back to the current or full display | Capture modes: active window, monitor under cursor, full screen, region. On Wayland "active window" may be impossible (spike S3). |
| Take the program's file path and climb to the folder with the same name; if none, use the folder above | Implemented and tested: `core/context/src/climb.rs`. "The folder above" is read as the program's own folder (its parent). The result is available as `{{project.folder}}`. |
| Read and parse some files, gather specific data | Data rules (Phase 3); only allow-listed paths, read-only (`core/context/src/glob.rs`). |
| Basic screenshot editing: crop, drawing, circle/square, text, colour | Added to the CargoFrame window in Phase 2/3. The editor works on the full-resolution capture and exports a flattened PNG when sharing. |
| Files found in the folder, shown with preselected options | Files block with `include` and `preselect` patterns relative to the project folder. |
| Each log action and each web API setup is its own file | Adopted: one JSON file per trigger, connection, data rule, info type and layout (see below). "Log action" is read as **data rule**; tell us if you meant something else. |
| Destinations: Jira, Reddit, Twitter, Claude, BlueSky | Jira, Reddit, Claude in v1. **Twitter/X and BlueSky are v1.1** (decided 2026-10-23). Custom API can cover either sooner when the API allows it. |

## Configuration files

```
%AppData%\CargoFrame\
  general.json          theme, run on startup, memory mode
  safe.json             safe settings
  triggers\<id>.json
  connections\<id>.json     secrets are NOT stored here
  rules\<id>.json           data rules
  infotypes\<id>.json
  layouts\<id>.json
```

Every file carries `schema_version`, is validated on load, and is migrated with a backup when the
version changes. One file per item keeps them easy to edit by hand, to share, and to review in
version control.

## Built-in variables (proposed)

Available to `{{...}}` in text blocks, prompts and info types, next to `{{rule.<id>}}` values:
`project.folder`, `project.name`, `app.exe`, `app.title`, `capture.date`, `capture.time`.
