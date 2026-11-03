<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/brand/convoy-logo-dark.svg">
    <img alt="CargoFrame" src="assets/brand/convoy-logo-light.svg" height="64">
  </picture>
</p>

CargoFrame is a small tray app for Windows 11 and Linux that shares what is on your screen, together
with the context around it.

Press a shortcut. CargoFrame captures the program you are in (a screenshot, or a screen recording),
finds that program's project folder, reads the few values you asked for (a version number, a log line,
the files you ticked), and opens a window where you check everything, draw on the picture, type a note
and press one button to send it to **Jira, Google Drive or Sheets, Reddit, Claude, Slack, Discord,
Teams** or any web service with an API. Nothing leaves your computer until you press that button.

> **Status: private beta.** All features are built and tested, but the app has not yet run on a real
> Windows or Linux desktop or with real accounts, and the installer is not signed. What is and is not
> verified is listed in [`docs/status.md`](docs/status.md).

---

## Install

CargoFrame installs for your own user only: no administrator password, nothing outside your own folders.

### Windows 11

1. On GitHub, open the repository, then **Actions**, then the workflow **Bundle**, then the latest run on
   `main`. Under **Artifacts**, download **windows-installer** (kept for 7 days; it downloads as a zip).
2. Unzip it and run `CargoFrame_..._x64-setup.exe`. Because the installer is not signed yet, Windows may say
   it is from an unknown publisher: choose **More info**, then **Run anyway**.
3. The installer asks one question, "Run CargoFrame when I sign in" (on by default). Finish, and look for the
   CargoFrame icon in the notification area at the bottom right (click the small arrow if it is hidden).

*Or build it yourself:* install [Rust](https://rustup.rs) and Node 22, then double-click
`scripts\windows\build.cmd`. The installer is written to `target\release\bundle\nsis\`.

Uninstall from Windows **Settings, Apps**. It asks whether to delete your settings too (the default is to keep them).

### Linux (x64)

1. Download the **linux-appimage** artifact from the same **Bundle** run and unzip it.
2. In that folder run `./install.sh` (add `--no-startup` if you do not want it to start when you sign in).
3. The tray icon needs a system tray. KDE has one; GNOME needs the AppIndicator extension. Without one,
   CargoFrame opens its Settings window at launch instead.

Uninstall with `./uninstall.sh` (it keeps your settings unless you add `--delete-data`).

### Optional: recording video

To record the screen, install ffmpeg once: on Windows `winget install Gyan.FFmpeg`, on Linux
`sudo apt install ffmpeg` (or your distribution's equivalent). Then open **Settings, Video** and press
**Test ffmpeg**. Screenshots do not need it.

---

## How to use it: the basics

CargoFrame starts empty: no connections, no shortcuts, no layouts. Set up three things once, then use it.

**1. Open Settings.** Left-click the tray icon (right-click shows the menu: the capture entries, Open
settings, Pause triggers, Quit).

**2. Add a connection** (*Connections, Add a connection*). Pick the service, fill in the form, and press
**Sign in** or **Save**, then **Test**. Passwords and tokens go to your system's password store, never to a
file. You can skip this step and start with **Save a capture locally**, which needs no account.

**3. Make a layout** (*Layouts, Start from a template*). A layout is the window that opens when you capture.
Press **Use** on a template, change what you like (drag blocks around, rename the button), and press
**Save as my layout**.

**4. Add a trigger** (*Triggers, Add a trigger*). Give it a name, click the Shortcut box and press the keys,
choose **Makes** (a screenshot or a video), what to capture (the program in focus, the screen under the mouse,
or all screens) and your layout, then **Save**.

**5. Capture.** Press the shortcut in any program. The CargoFrame window opens with the picture, the text
boxes and dropdowns, and the files it found. Crop or draw on the picture, type your note, tick the files you
want, and press the sharing button at the bottom. If sending fails it retries, then keeps a zip of everything
so nothing is lost. A button with no connection says **Save locally** and saves that zip into the folder you
chose in *General, Safety*.

**Video:** a trigger whose *Makes* is *A video* starts recording the first time you press its shortcut and stops
the second time. The window then plays the clip, and you can trim it (drag the two handles under the clip, or
press *Start here* and *End here* while it plays), crop it, and put one red ring with a text on it before sending.
Only the last seconds are kept (10 by default, set in *Settings, Video*).

**Pulling values from files (optional):** under *Data from files* you describe a value to read from a file in the
program's project folder (JSON, YAML, TOML, INI, CSV, XML, a regular expression, or a line of a text or log file),
and use it in your text boxes and dropdowns. CargoFrame only reads files these rules allow.

**Make it yours:** *Settings, General, Appearance* sets the theme (system, light or dark) and the **accent
colour**, which colours the buttons, focus rings, links, the selected block, the arrow of the logo and the arrow
of the tray icon. Pick one of the suggestions, use the colour picker, or type a code like `#4f8ee0`.

**Pause or stop:** *Pause triggers* in the tray menu turns all shortcuts off without deleting anything.

Planned for a later version: Twitter/X, BlueSky and Instagram.

---

## Ready-made setups and connections

**Nothing is pre-installed.** There are no ready-made connections, triggers or layouts sitting in your settings.
What CargoFrame ships is a set of **templates**: ready-made windows you can start a layout from.

### The templates

A template is offered only for the services you have already added under *Connections* (the two local ones are
always there):

| Service | Templates |
|---------|-----------|
| No account | Save a capture locally · Save a recording locally |
| Jira | Jira bug report · Jira bug report with a recording · Jira comment on an issue |
| Google | Google Drive upload · Google Drive upload of a recording · Google Sheet row |
| Reddit | Reddit post |
| Claude | Ask Claude about a capture (it hands the picture and your prompt to a chat; there is no chat inside CargoFrame) |
| Slack | Slack message · Slack message with a recording |
| Discord | Discord message |
| Teams | Teams message |
| Custom API | Custom API post |

Recordings can be sent to Slack, Google Drive, Jira and Discord (up to 100 MB; the service may allow less). Reddit,
Teams, Claude and Custom API take pictures only, and the window says so.

### The connections

Eight kinds can be added: **Jira**, **Google** (Drive, optionally Sheets), **Reddit**, **Claude**, **Slack**,
**Discord**, **Teams** and **Custom API**. Add only the ones you use; the rest simply do not appear.

### Keeping only a few

A template itself is never stored or changed. Opening one shows an unsaved layout, and **Save as my layout** writes
your own copy. So there is nothing to uninstall: **do not save the templates you do not want**, and delete the
copies you no longer use. To prune what you have:

| What | How to remove it | Gentler option |
|------|------------------|----------------|
| A layout | *Layouts*, choose it in the list, **Delete**, then **Delete for good** (**Keep** cancels) | none needed |
| A trigger | *Triggers*, **Delete**, then **Delete for good** | switch off **Enabled** in **Edit** |
| A connection | *Connections*, **Delete**, then **Delete for good**. Its passwords and tokens are removed from the password store too | **Sign out**, or switch off **Enabled** |
| A data rule or a destination field | *Data from files* / *Destination fields*, **Delete**, then **Delete for good** | none needed |

A small starting set that covers most people: **Save a capture locally** (no account), plus the one template for
the service you really use (for example **Jira bug report** or **Ask Claude about a capture**), each with one trigger.

Before a big clean-up, use *General, Export and import*: tick what to keep, press **Export**, and you get one file
that **Import** puts back later. To start completely fresh, quit CargoFrame and delete its settings
folder (`%AppData%\CargoFrame` on Windows, `~/.config/CargoFrame` on Linux), or let the Windows uninstaller delete
settings.

---

## What is in this repository

| Path | Contents |
|------|----------|
| `app/` | The Tauri 2 program: tray, shortcuts, windows, commands, installer hooks |
| `ui/` | The Svelte 5 windows: Settings and CargoFrame (one page each) |
| `core/context` | Project-folder climb, `{{variable}}` templates, read-only allow-list |
| `core/safe` | Retry with backoff, failure classes, fallback ladder, zip naming |
| `core/layout` | Window layout model, grid, validation chips, ready-made templates |
| `core/handoff` | Claude chat links, clipboard plan, pinned and recent chats |
| `core/config` | One JSON file per trigger, connection, rule, info type and layout; migration |
| `core/capture` | Screen and window capture (Windows, X11, Wayland portal), picture editing |
| `core/window` | Which program is in focus, and its path |
| `core/rules` | Data rules: read allowed files and pull out values |
| `core/bundle` | Zip files for the fallback and the Files block |
| `core/net` | The one HTTP client: https checks, checked redirects, multipart, mock server |
| `core/connectors` | Jira, Google, Reddit, Claude, Slack, Discord, Teams, Custom API, OAuth, the secret store |
| `core/video` | Video: the recorder (ffmpeg), the ring of segments, the edit model and the export |
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

Rust stable (see `rust-toolchain.toml`), Node 22. On Windows, `scripts\windows\build.cmd` builds the program and
the installer (`build.cmd program` skips the installer, `build.cmd install` opens it afterwards, `fast` skips
reinstalling the web packages; VS Code's **Ctrl+Shift+B** runs the same). It is a cmd script so that PowerShell's
"running scripts is disabled" setting cannot stop it. On Linux the app needs the WebKitGTK and GTK 3 development
packages (the list is in `.github/workflows/ci.yml`). Run the program with `cd app && ../ui/node_modules/.bin/tauri dev`.
To try the windows alone in a browser with demo data, run `npm run dev` in `ui/` and open `settings.html` or `capture.html`.

## Documents

- [Decisions](docs/decisions.md): what was decided, and how the project brief changed the plan
- [Status](docs/status.md): what works, what is verified, what is still open
- [Security](docs/security.md): the review, the findings and the known limits
- [Performance](docs/performance.md): budgets, how each is checked, what has been measured
- [Spikes](docs/spikes.md): the experiments to run on real machines
- [Signing](docs/signing.md): certificates and update keys (never commit these)
- [Installers](installers/README.md): what the Windows and Linux packages do
