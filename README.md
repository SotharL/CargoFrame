<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/brand/convoy-logo-dark.svg">
    <img alt="CargoFrame" src="assets/brand/convoy-logo-light.svg" height="64">
  </picture>
</p>

**CargoFrame shares what is on your screen, with the context around it, in one shortcut.**

Press a shortcut. CargoFrame captures the program you are in (a screenshot or a short recording), finds that
program's project folder, reads the few values you asked for (a version number, a log line, the files you
ticked), and opens one window. You check it, draw on the picture, type a note, and press one button to send it to
**Jira, Google Drive or Sheets, Reddit, Claude, Slack, Discord, Teams** or any web service with an API.
Nothing leaves your computer until you press that button.

For Windows 11 and Linux. It waits in the tray and costs almost nothing while it waits.

> **Status: private beta.** Everything is built and tested, but it has not yet run on a real Windows or Linux
> desktop or with real accounts, and the installer is not signed. [What is and is not verified](docs/status.md).

---

## Install

CargoFrame installs for your own user only: no administrator password, nothing outside your own folders.

### Windows 11

1. On GitHub open **Releases** and download **CargoFrame-Setup.exe**.
2. Double-click it and press **Install** (**Update** if CargoFrame is already there). That is the whole installer.
3. Windows may say the publisher is unknown, because the installer is not signed yet: choose **More info**, then **Run anyway**.
4. CargoFrame opens on its **Setup** page. It lists what it can use on your computer and has one button for what is missing.

CargoFrame waits in the notification area (bottom right; click the small arrow if it is hidden) and starts when
you sign in. You can turn that off in **Settings, General**.

<details>
<summary><b>Silent install, building it yourself, uninstalling</b></summary>

- **Silent install (scripts):** run the installer with `/S`. Add `/NOSTARTUP` to leave "start when I sign in" off.
- **Build it yourself (developers only):** install [Rust](https://rustup.rs) and Node 22, then run
  `scripts\windows\build.cmd`. The installer is written to `target\release\bundle\nsis\`.
- **Uninstall:** Windows **Settings, Apps**. It asks whether to delete your settings too (the default is to keep them).

</details>

<details>
<summary><b>Linux (x64)</b></summary>

1. Download the **linux-appimage** file from the **Bundle** run on GitHub (Actions) and unzip it.
2. In that folder run `./install.sh` (add `--no-startup` if you do not want it to start when you sign in).
3. The tray icon needs a system tray. KDE has one; GNOME needs the AppIndicator extension. Without one,
   CargoFrame opens its Settings window at launch instead.

Uninstall with `./uninstall.sh` (it keeps your settings unless you add `--delete-data`).

</details>

<details>
<summary><b>Video needs ffmpeg (screenshots do not)</b></summary>

Open **Settings, Setup**. On Windows the button **Install ffmpeg** installs it through Windows' own package manager
(winget, for your user only, no administrator password). On Linux the page shows the one command to copy, for example
`sudo apt install ffmpeg xdotool`, because Linux asks for your password to install programs. The Setup tab shows
how many programs are missing until they are in place.

</details>

---

## Your first capture in five steps

Do these once. The app starts empty: no connections, shortcuts or layouts.

1. **Open Settings.** Left-click the tray icon.
2. **Add a connection** (*Setup, Connections, Add a connection*). Pick the service, fill in the form, press **Save**,
   then **Test**. Beside each token field a button opens the service's own page to get it. You can skip this and
   start with **Save locally**, which needs no account.
3. **Make a layout** (*Layouts, Start from a template*). A layout is the window a capture opens. Press **Use** on a
   template, change what you like, and press **Save as my layout**.
4. **Add a trigger** (*Triggers, Add a trigger*). Name it, click the Shortcut box and press the keys, choose what it
   **Makes** (a screenshot or a video), what to capture, and your layout. **Save**.
5. **Press the shortcut** in any program. Check the window, type your note, press the sharing button.
   If sending fails it retries, then keeps a zip of everything so nothing is lost.

> **Stuck? Use the guides.** At the bottom of the Settings menu, **Guides** walks you through a setup: it opens the
> right page, draws a box around the field and says in one sentence what goes in it. There is one for each service,
> and for your first capture, *Data from files* and *Destination fields*.

---

## Connect a service

Add only the services you use. Each form has a **Guide** link, and a button next to each token field opens the page
where you get it.

| Service | What you need | Good to know |
|---------|---------------|--------------|
| **Jira** | Your Jira address, and an API token (Cloud, with your e-mail) or a personal access token (Server / Data Center) | The reference service: projects, issue types, priority, labels, attachments |
| **Google** | A Client ID from your own Google Cloud project ("Desktop app") | Uploads to Drive; can also add a row to a spreadsheet |
| **Reddit** | A Client ID from an "installed app" at reddit.com/prefs/apps | Posts a picture |
| **Claude** | Nothing | Opens a chat by link and pastes your picture and note; the desktop app or the browser. Setup can install the desktop app on Windows |
| **Slack** | A bot token (`xoxb-`) from an app you make in your workspace | Invite the app to each channel it posts in |
| **Discord** | A webhook link for each channel (no bot needed) | A bot token is the last resort, for people who want the server lists |
| **Teams** | An app ID from your own Microsoft Entra app (work or school accounts) | Posts a message |
| **Custom API** | An address and how to sign in | Any web service with an API |

Passwords and tokens go to your system's password store, never to a file.

<details>
<summary><b>Discord in more detail</b></summary>

- **The easy way:** in Discord open the channel's settings, **Integrations, Webhooks, New Webhook, Copy Webhook URL**,
  and paste it into CargoFrame. One link per channel you post to. No bot, no developer portal.
- A webhook cannot list servers, channels or threads, so the channel is one of the links you added. For a **forum**
  channel, CargoFrame posts a new thread by its **title**; a short convention such as `[PROJ-123] short title`
  makes threads easy to find. To reply in an existing thread, paste its link (**Copy Link** in Discord).
- Posting as your own Discord account is not offered: Discord does not allow it and it can get the account closed.
- The bot way (a bot token) lets CargoFrame list servers, categories and channels in dropdowns. Use it only if you need that.

</details>

<details>
<summary><b>Dropdowns that depend on each other, and last choices</b></summary>

In the layout editor a dropdown can be **narrowed by** another one: server, then category (optional), then channel or
forum. A level you leave empty is skipped. Where a service gives no list, you type or paste the value and the box
searches what it knows. With no destination at all, the window saves a zip on your computer instead.

Each dropdown remembers its **last five choices** on this computer (only the values, never secrets), shows the last one
used first, and starts on it.

</details>

---

## What the window can do

<details>
<summary><b>Which button sends what (tick boxes)</b></summary>

A window can have up to five sharing buttons. Each block shows a small row of tick boxes on its top edge, one per
button, in that button's colour. Clear a box to leave the block out of that button's share this time, or tick one the
button had switched off. A dashed box cannot be ticked (hover it to see why), and a box with a ring is needed by the
button. Pointing at a button outlines everything it sends. In the layout builder, choose for each text or dropdown
block whether a button sends it, as the prompt, or in one of the service's fields.

</details>

<details>
<summary><b>Video</b></summary>

A trigger whose **Makes** is *A video* starts recording the first time you press its shortcut and stops the second
time. Only the last seconds are kept (10 by default, set in *Settings, Video*). The window plays the clip; you can trim
it (drag the two handles under it), crop it (*Crop*, drag the box, *Apply crop*) and put one red ring with a text on
it (*Mark*) before sending.

Click the Video block in the layout builder to choose what is recorded: the whole screen or the window in focus, the
**sound of the computer** (on by default) and a **microphone** (off by default).

- **Screenshot and video together:** tick *Also take a screenshot when the recording ends* on the video block, or press
  *Take screenshot* while you look at the clip. The screenshot block stays empty until then.
- **Where a recording can go:** Slack, Google Drive, Jira and Discord (up to 100 MB; the service may allow less).
  Reddit, Teams, Claude and Custom API take pictures only, and the window says so.
- **Sound on Windows:** ffmpeg can record the computer's sound only through a loopback device. In the Windows sound
  settings (Recording tab, right-click, Show disabled devices) turn on **Stereo Mix**. **Setup** shows the current
  sound output and says whether videos can include it.

</details>

<details>
<summary><b>Pulling values from files (optional)</b></summary>

Under **Data from files** you describe a value to read from a file in the program's project folder (JSON, YAML, TOML,
INI, CSV, XML, a regular expression, or a line of a text or log file) and use it in your text boxes and dropdowns as
`{{name}}`. CargoFrame only reads files that your rules allow, and only inside that folder.

</details>

<details>
<summary><b>Several windows, pause, and making it yours</b></summary>

- **Several windows:** pressing a shortcut while a window is open opens another, up to ten, each with its own picture and choices.
- **Pause or stop:** *Pause triggers* in the tray menu turns every shortcut off without deleting anything.
- **Look:** *Settings, General, Appearance* sets the theme (system, light, dark) and an **accent colour** for buttons,
  focus rings, the selected block and the logo arrow.

</details>

---

## Ready-made layouts, and keeping only a few

Nothing is pre-installed: no ready-made connections, triggers or layouts sit in your settings. CargoFrame ships
**templates** you can start a layout from, offered only for the services you added.

<details>
<summary><b>The templates</b></summary>

| Service | Templates |
|---------|-----------|
| No account | Save a capture locally · Save a recording locally |
| Jira | Bug report · Bug report with a recording · Comment on an issue |
| Google | Drive upload · Drive upload of a recording · Sheet row |
| Reddit | Post |
| Claude | Ask Claude about a capture (no chat inside CargoFrame: it hands the picture and your prompt to a chat) |
| Slack | Message · Message with a recording |
| Discord | Message |
| Teams | Message |
| Custom API | Post |

A template is never stored or changed. Opening one shows an unsaved layout; **Save as my layout** writes your copy.

</details>

<details>
<summary><b>Removing things</b></summary>

| What | How | Gentler option |
|------|-----|----------------|
| A layout | *Layouts*, pick it, **Delete**, then **Delete for good** | none needed |
| A trigger | *Triggers*, **Delete**, then **Delete for good** | switch off **Enabled** |
| A connection | *Setup, Connections*, **Delete**, then **Delete for good** (its tokens leave the password store too) | **Sign out**, or switch off **Enabled** |
| A data rule or destination field | **Delete**, then **Delete for good** | none needed |

Before a big clean-up use *General, Export and import*: tick what to keep, press **Export**, and **Import** puts it back
later. To start completely fresh, quit CargoFrame and delete its settings folder (`%AppData%\CargoFrame` on Windows,
`~/.config/CargoFrame` on Linux).

</details>

---

## Problems

<details>
<summary><b>No tray icon</b></summary>

GNOME shows no tray icons without the AppIndicator extension. Until then, run CargoFrame again to open Settings.

</details>

<details>
<summary><b>Windows says "unknown publisher"</b></summary>

The installer is not signed yet. Choose **More info**, then **Run anyway**.

</details>

<details>
<summary><b>The video has no sound, or no video at all</b></summary>

No video: open **Settings, Setup** and install ffmpeg. No computer sound on Windows: turn on **Stereo Mix** (see Video above).
The notification after a recording says why a sound was left out.

</details>

<details>
<summary><b>Claude opened but nothing was pasted</b></summary>

Leave Claude in front for a few seconds while it works, and raise **Extra seconds to wait before pasting** in the Claude
connection if the chat is slow to open. Turn off **Send the message for me** to look at the message first.

</details>

<details>
<summary><b>A send failed</b></summary>

CargoFrame retries, then keeps a zip of everything it was going to send and tells you where. Nothing is lost; the message
says what the service answered (a missing permission, a file too large, a deleted webhook).

</details>

---

## For developers

<details>
<summary><b>What is in this repository, and how to build and test it</b></summary>

| Path | Contents |
|------|----------|
| `app/` | The Tauri 2 program: tray, shortcuts, windows, commands, installer hooks |
| `ui/` | The Svelte 5 windows: Settings and CargoFrame (one page each) |
| `core/context` | Project-folder climb, `{{variable}}` templates, read-only allow-list |
| `core/safe` | Retry with backoff, failure classes, fallback ladder, zip naming |
| `core/layout` | Window layout model, grid, validation, ready-made templates |
| `core/handoff` | Claude chat links, clipboard plan, pinned and recent chats |
| `core/config` | One JSON file per trigger, connection, rule, field and layout; migration |
| `core/capture`, `core/window` | Screen and window capture (Windows, X11, Wayland portal); which program is in focus |
| `core/rules` | Data rules: read allowed files and pull out values |
| `core/bundle` | Zip files for the fallback and the Files block |
| `core/net` | The one HTTP client: https checks, checked redirects, multipart, mock server |
| `core/connectors` | Jira, Google, Reddit, Claude, Slack, Discord, Teams, Custom API, OAuth, the secret store |
| `core/video` | The recorder (ffmpeg), the ring of segments, the edit model, the export |
| `docs/` | Design, decisions, status, security, performance, signing, spike runbook |
| `scripts/` | CI helpers, end-to-end and timing checks, signing helpers |
| `installers/` | Windows (NSIS) and Linux (AppImage and install script) |

**Build and test.** Rust stable (see `rust-toolchain.toml`) and Node 22.

```
cd ui && npm ci && npm run build && cd ..   # the windows must be built once for the app
scripts/dev/check.sh quick                  # fast: format, clippy, tests, svelte-check, UI tests
scripts/dev/check.sh all                    # everything, including budgets and a real capture on a virtual screen
```

On Windows, `scripts\windows\build.cmd` builds the program and the installer (`program` skips the installer, `install`
opens it afterwards, `fast` skips reinstalling the web packages; VS Code's **Ctrl+Shift+B** runs the same). On Linux the
app needs the WebKitGTK and GTK 3 development packages (the list is in `.github/workflows/ci.yml`). Run it with
`cd app && ../ui/node_modules/.bin/tauri dev`. To try the windows alone in a browser with demo data, run `npm run dev`
in `ui/` and open `settings.html` or `capture.html`.

</details>

<details>
<summary><b>Documents</b></summary>

- [Design](docs/design.md): how the app is built today, in one short page
- [Decisions](docs/decisions.md): what was decided, and why
- [Status](docs/status.md): what works, what is verified, what is still open
- [Security](docs/security.md): the review, the findings and the known limits
- [Performance](docs/performance.md): budgets, how each is checked, what has been measured
- [Spikes](docs/spikes.md): the experiments to run on real machines
- [Signing](docs/signing.md): certificates and update keys (never commit these)
- [Installers](installers/README.md): what the Windows and Linux packages do

</details>

Planned for a later version: Twitter/X, BlueSky and Instagram.
