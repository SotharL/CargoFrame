# Status

_Last updated 28 Nov 2026 · version 0.8.0 · public, MIT OR Apache-2.0._

What is verified today, how, and what is not. What each version changed is in the [changelog](changelog.md); why,
in [decisions](decisions.md).

**In one line:** CargoFrame runs every day on the owner's Windows 11 PC, sends real Jira tickets, GitHub issues and
Discord messages, and passes about 1,850 tests on every push; Linux is verified on virtual screens only, and the
services other than Jira, GitHub and Discord only against mocks of their documented APIs.

---

## Verified automatically, on every push

| Area | How | Where |
|------|-----|-------|
| Logic | 1,168 Rust tests: rules, layouts, templates, connectors, retries, migrations, updates, security regressions | `cargo test` |
| Services | Each connector against a mock of its documented API: sign-in, lists, send, retry, never twice; Jira Cloud's real create-metadata answers kept as cases | `core/connectors/src/*/tests.rs` |
| Windows (UI) | 684 tests on the in-memory backend; whole windows rendered with demo data; guides walked; search entries point at real code | `vitest` |
| How the windows look | Every Settings page and the CargoFrame window in Chrome, smallest and large, light and dark: nothing outside its box, no file missing | `ui/scripts/overflow-check.mjs` |
| Rust and the windows in step | Settings types and defaults generated from Rust; mirror tests for the lists copied by hand; value-finding cases run on both sides | `ui/src/lib/generated`, `mirrors.test.ts`, `pick_cases.json` |
| Whole program, Linux | The real binary captures a fake program on a virtual X11 screen and on a headless sway desktop (Wayland and XWayland, GNOME-like shortcuts); a second sign-in start is handed over; no error in the log | `scripts/e2e/` |
| Windows build | Rust tests and clippy on Windows; installer silent install, upgrade, uninstall and a click on its window | `ci.yml`, `bundle.yml`, `release.yml` |
| Budgets | Idle memory and CPU, capture and window times, script size | `scripts/ci/check-budgets.mjs`, timing tests |
| Languages | Every text of both codebases in French, Spanish, Italian and German, with the same `{names}` | `scripts/i18n/check.mjs` |
| Dependencies | `cargo audit`, `npm audit`; licenses within the accepted list | `security.yml`, `scripts/licenses/about.toml` |

## Seen on real machines

| Where | What works |
|-------|------------|
| **Owner's Windows 11 PC**, since 7 Nov | Install and in-place updates (0.2 to 0.7.3); tray; shortcuts, F-keys and number-pad combinations; screenshots of the window in focus; recordings of the window in focus with sound; ffmpeg downloaded from System; the CargoFrame window; debug mode and its bug reports |
| **Jira Cloud** (owner's site) | Tickets created with the picture; the people search; the create metadata read through the Atlassian connector (a Task needs nothing more; a Subtask a parent) |
| **GitHub** (this repository) | Issues created with pictures in the attachments branch, from the bug-report layout |
| **Discord** | Messages sent through a webhook |

## Not verified yet

| Area | What is missing |
|------|-----------------|
| Linux desktops | A real GNOME, KDE and Hyprland session (trays, the portal's dialogs, KDE's shortcut dialog, `wtype`); the Secret Service credential store (an in-memory stand-in in tests) |
| Services | Real accounts for Google, Reddit, Teams (each needs an app registered by the person), Slack (a bot token), X, Bluesky, Instagram, Claude's composer with a pasted picture; Discord threads and bot lists. New in 0.8.0 and tried against the services' documented APIs only: videos to X, Bluesky and Instagram, Teams files in a channel and a chat, Reddit's post requirements |
| Jira | A ticket sent from CargoFrame with the required-fields check on; Jira Server and Data Center, and its OAuth sign-in through an application link |
| Company setup | A managed folder put in place by Intune or Group Policy on a real PC (tested with temporary folders and `CARGOFRAME_MANAGED`); the Custom API's OAuth with a real identity provider |
| Windows | *Check this PC* on a real PC; sound of one program; several screens and 150% scaling while recording; the winget install of ffmpeg; *Stop the older copy* (administrator) |
| Performance | The budgets on a reference machine (cold windows were slightly over budget on a virtual screen) |
| Releases | A signing certificate (`scripts/windows/sign.ps1`, `new-signing-material.ps1` never run with one) |

## Checks to run by hand

On a real PC, with *Debug mode* on (Settings › System), press **Check this PC** first: it tries the screenshot, a
3-second recording, the apps, starting at sign-in, the credential store and the shortcuts, and keeps a report for the
next bug report. Then:

| Check | Expect |
|-------|--------|
| Install `CargoFrame-Setup.exe` over an older copy | The button says Update; settings kept; one sign-in entry |
| A shortcut on a program with a project folder | The window opens with its values and files |
| A recording with sound, then *Take screenshot*, then send | The kept part and area arrive, with the marks and the picture |
| Two shortcuts one after the other | Two windows, each sending its own capture |
| Each service you use: Test, then send | The link to what was made; with the network off, a zip kept |
| Settings in each language and at the smallest size | Nothing cut, nothing outside its box |
| A light and a dark taskbar, two accent colours | The tray arrow and buttons follow the accent |
| On Linux: GNOME, KDE, sway or Hyprland | Shortcut, focus, capture; the portal asks once |
