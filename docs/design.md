# Design

How CargoFrame is built today (7 Nov 2026). It is a short map, not a manual: for why a choice was
made read [decisions.md](decisions.md), for what has been tried on real machines read [status.md](status.md).

## What it is

A tray program for Windows 11 and Linux. A shortcut captures the program in focus (a picture or a
recording), finds that program's project folder, reads the few values the person asked for, and opens one
window to check everything and send it to a web service with one button. Nothing is sent without that button.

Three rules shape everything else:

1. **Cheap while idle.** The always-on process is tray-only: no web view, a few megabytes.
2. **Easy for the person.** Every setup step has a button, a link or a short in-app guide. The person never edits a file.
3. **Little to maintain.** One place for each thing: one list of templates, one list of "get a token" links,
   one file per setting, one HTTP client, one data-driven guide. A service that changes breaks one file.

## One press, from shortcut to sent

```
shortcut ─▶ which window is in focus? ─▶ capture (picture, or start/stop a recording)
         ─▶ climb from the program's path to its project folder
         ─▶ run the data rules on the allowed files in that folder
         ─▶ open the CargoFrame window (a layout: picture, texts, dropdowns, files, video, buttons)
         ─▶ the person edits, ticks and presses a sharing button
         ─▶ the connector sends (retry, then a kept zip if it still fails)
```

- **Capture** (`core/capture`, `core/window`): Windows (a copy of the desktop, so menus and pop-ups are in the picture), X11, and the Wayland portal. The
  program in focus gives its executable path; with no focus the screen under the mouse is used.
- **Project folder** (`core/context`): climb the path to a folder with the same name as the program, else one
  folder above. Only files a data rule allows are read, and never outside that folder.
- **Data rules** (`core/rules`): read a value from JSON, YAML, TOML, INI, CSV, XML, a regular expression, or a line
  of a text or log file. The value fills `{{variables}}` in text boxes and dropdowns.
- **Sharing** (`app/src/share.rs`, `core/connectors`): builds one `Payload` (picture, optional still screenshot,
  video, files zip, texts, dropdown choices) from what each button's blocks are ticked to send, then hands it to the connector.
- **Safe fallback** (`core/safe`, `core/bundle`): a failed send is retried with backoff; if it still fails, a zip of
  everything is kept so nothing is lost. A button with no connection says *Save locally* and writes that zip.

## Pieces

| Piece | Where | Job |
|-------|-------|-----|
| Tray program | `app/` (Tauri 2) | Tray, shortcuts, windows on demand, Tauri commands, the secret store, installer hooks |
| Windows (UI) | `ui/` (Svelte 5) | `settings.html` and `capture.html`, one page each, built once, loaded on demand |
| Logic crates | `core/*` | Everything testable without a display: no Tauri, no window |
| Installers | `installers/` | Windows NSIS (one button), Linux AppImage and `install.sh` |
| CI | `.github/` | Linux CI on pull requests; Windows and Linux on `main`; installer build and click test; release draft |

Side effects (files, the secret store, the network, the clock) sit behind traits, so tests use a local mock
server and an in-memory secret store. `unsafe` code is forbidden in every crate.

## Settings are plain files

One JSON file per item under the settings folder (`%AppData%\CargoFrame`, `~/.config/CargoFrame`):
`general.json`, `safe.json`, `video.json`, and the folders `triggers/`, `connections/`, `rules/`,
`infotypes/`, `layouts/`. Every file carries `schema_version`, is checked when it is read, and is migrated with
a backup. Secrets (tokens, passwords, webhook links) are **not** in these files; they are in the system credential store.

**Last choices** in a dropdown (the last five, the newest first) are kept in the window's own local storage,
per layout and block: values and labels only, never secrets.

## The CargoFrame window and layouts

A **layout** is a grid of blocks: Screenshot, Video, Text, Dropdown, Files, and up to five sharing buttons. Ready-made
**templates** (`core/layout/src/templates.rs`) are offered for the services you have connected and are never stored
until you press *Save as my layout*.

- **Screenshot and Video can be in one layout.** The video block has *Also take a screenshot when the recording
  ends*; the screenshot block can stay empty until then, and the video block has a *Take screenshot* button.
  Services that take both (Jira, Slack, Discord, Google, and the local zip) get the video and the still; Teams, Reddit,
  Claude and Custom take pictures only, so there the screenshot is sent if the video box is unticked.
- **Tick boxes** on each block say which buttons send it. Dashed = this destination cannot take it; ring = required.
- **Dropdowns** can come from a destination (Jira projects, Discord servers, ...). A dropdown can **narrow by**
  another one (server, then category, then channel or forum), be **optional** (skipped level), and **remember**
  its last choices. A dropdown on a destination with no list takes a typed or pasted value.

## Connections

One connector per service, in `core/connectors/src/<service>`. Each answers the same five questions
(what can I take, test, list the options, send, and what did the person get back), so the share flow never
names a service.

| Service | Sign-in | What it sends | Video |
|---------|---------|---------------|-------|
| Jira Cloud / Server | E-mail and API token / personal access token | An issue or a comment, with the picture, still, video and files | Yes |
| Google | OAuth, the person's own Desktop client | Drive upload, optional Sheets row | Yes |
| Slack | Bot token (`xoxb-`) | A message with files, in a channel | Yes |
| Discord | **A webhook link per channel (no bot)**, or a bot token as a last resort | A message with files; a forum post by title; a reply in a thread | Yes (size limit of the server) |
| Reddit | OAuth, the person's own app | A post with the picture | No |
| Teams | OAuth through the person's own Entra app (work or school) | A message | No |
| Claude | None: opens a chat by link and pastes | The picture, files and prompt, into the Claude desktop app or the browser | No |
| Custom API | Header or token | Any HTTPS endpoint, with the fields you define | No |

- **Discord without a bot.** A webhook is made in the channel's settings and pasted once. It cannot list servers,
  channels or threads, so those are typed, or pasted as a link (*Copy Link* in Discord). A forum is posted to by
  *thread name* (a title convention such as `[PROJ-123] short title`), and an existing thread by its link.
  Posting through the person's own Discord account is **not** built: it breaks Discord's terms and risks the account.
- **Get the token.** Next to each sign-in field a button opens the service's own page (`ui/src/lib/connectorHelp.ts`
  is the only place those addresses live).
- **A local program a connector needs** (the Claude desktop app) is checked on the Setup page; on Windows one button
  installs it with winget, and the download page is always one click away.
- **OAuth** (`core/connectors/src/oauth.rs`): PKCE on a loopback address, tokens kept in the credential store.
  Google, Reddit and Teams need the person's own app ID: CargoFrame does not ship one.
- **Network** (`core/net`): the one HTTP client. https only, checked redirects, a size cap, retries, multipart.

## Video

`core/video` runs **ffmpeg** (the one optional program) to keep a ring of short segments, so a press of the shortcut
keeps the last seconds (10 by default). The window plays the clip, trims it (timeline handles), crops it and puts
one ring with a text on it; the export is cut by ffmpeg. Sound of the computer and a microphone are optional; if
a sound device is missing the recording goes on without it and says why. Windows' computer sound needs a loopback
device (Stereo Mix).

## Setup, installer and updates

- **Installer:** a window with one button (*Install*, or *Update*), per user, no administrator password, and it
  opens CargoFrame on the Setup page.
- **Settings, Setup** (top half): what CargoFrame uses on this computer (ffmpeg, the sound output, the Claude
  app, `xdotool` on Linux), with one install button on Windows (winget, fixed arguments, one at a time). The
  **default sound output** is read from Windows when the page opens. **Connections** are the lower half of the
  same page.
- **Updates:** a manual "check for updates" against GitHub (a read-only token while the repository is private); a
  daily check is off by default.

## The in-app guide

`ui/src/settings/guide/` is a small data-driven tour: `tours.ts` lists, for each connector and for *first capture*,
*Data from files* and *Destination fields*, a few steps of `{page, field, one short sentence}`. The guide opens the
right page, scrolls to the field (found by its `data-tour` attribute), draws a box in the accent colour around it and
shows the sentence. A test fails if a step names a field that no longer exists, so the guides cannot silently go stale.

## Safety

Everything the program reads or sends is listed in [security.md](security.md). The short version: no telemetry; secrets
only in the credential store; files read only inside the project folder and only where a rule allows; every request
is https and to an address the person set; text from a screen or a file never reaches a command line; every outgoing
message is built from what is ticked, shown in the window first.

## Performance

Budgets, how each is checked and what was measured are in [performance.md](performance.md). In short: the tray-only
process stays under 40 MB, capture is ready in well under 150 ms, and each window loads under 250 KB of script.
The gate (`scripts/dev/check.sh all`) checks the size budgets and runs a real capture end to end.

## Testing

| Layer | How |
|-------|-----|
| Rust logic | Unit and integration tests per crate; connectors against local mock servers |
| Windows (UI) | vitest on an in-memory stand-in for the backend (`memoryApi`), plus browser demos (`?demo=...`) |
| Whole program | A fake program on a virtual X11 screen is captured by the real binary (`scripts/e2e/`) |
| Installers | Windows: silent install, upgrade, uninstall, and a click test of the real window |
| Guides | `tours.test.ts` (fields exist) and `walk.test.ts` (every step can be walked) |

One command runs the local gate: `scripts/dev/check.sh quick|rust|all`. GitHub minutes are used in batches,
not on every change.

## Known limits

- Not yet run on a real Windows or Linux desktop, nor against real accounts; see [status.md](status.md).
- Wayland "active window" may not be possible on some compositors (the fallback is the screen under the cursor).
- The installer and the program inside it are not signed yet.
- Twitter/X, BlueSky and Instagram are planned for a later version.
