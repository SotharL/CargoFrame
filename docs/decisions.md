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
  opens a chat by link and pastes the capture into its window (see "Sending into a Claude chat"
  below). The dropdown lists pinned
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

- **Memory modes** (Settings, General, Performance, Memory). *Low memory* destroys a window when it is closed.
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

## Settings update (after the first run of V1)

Requested by the owner after trying the app; each point and how it was read:

- **Settings order.** General first, then Connections, Triggers, Data rules, Info types, Layouts.
  The old *Safe* page is gone: its options live in General, under **Safety** (what happens when a
  share fails, *Folder for saved zips*, *Retries*), placed between Startup and Performance. A
  link to the old page still lands on General. *Memory* and the last measured timings are one
  **Performance** block; *Updates* and *Files* stay separate.
- **"Active connection".** A button has one when the connection it names exists and is signed in.
  An expired sign-in still counts, because the service may accept a retry and the failure screen
  offers Reconnect. A button with no connection chosen, a deleted connection, or a connection
  that is not signed in **saves locally**.
- **Save locally.** The button says "Save locally" and needs *Folder for saved zips*; with the
  folder empty or unwritable it stays disabled and says why. It writes the same zip a failed
  share keeps (screenshot, description, ticked files, manifest; never secrets) and sends nothing.
  Two saves in the same second never overwrite each other.
- **Default layouts are templates.** Eight built-in, read-only templates (a local save, Jira bug,
  Jira comment, Drive upload, Sheet row, Reddit post, Claude prompt, Custom API) are offered per
  connection. Opening one makes an unsaved layout; saving it, or pressing *Save as copy* on any
  layout, writes a **new file** and never touches the template or the original.
- **Plain text and log files in data rules.** A new *Text or log file* format with short
  instructions (`line`, `lines`, `head`, `tail`, `contains`, `after`, `level`) instead of regular
  expressions; colour codes are removed, big logs are read from the start or the end only.
- **Export and import.** One JSON file holds any mix of layouts, data rules and info types.
  Connections, secrets, triggers and shortcuts are never exported. Import is two steps: a preview
  (new / already here / cannot be imported, with notes such as a missing connection), then a
  choice per item: leave mine, replace mine, or import as a copy. Data rules cannot be imported as
  a copy because layouts refer to them by id. Every imported item passes the same checks as one
  typed into Settings.

## Feedback round 1: sending into a Claude chat (3 Nov 2026)

- **One click, everything at once.** The Share button of a Claude connection opens the chat (a new
  one, or the pinned chat), waits until the Claude window is really in front, then pastes the
  screenshot and the ticked files as files, pastes the message (title, prompt, the captured
  window's name, the file names) and presses Enter. There is no "Copy prompt" step any more. A
  link cannot carry any of this: Claude's own documentation only lets a link open a chat, with a
  short text for a new one.
- **It types into another program, so it checks first.** Nothing is pasted until the window in
  front is the Claude app or a known browser showing a Claude page; a terminal titled "claude" is
  never typed into. Before every paste and before Enter the window in front is looked at again and
  has to be the same window. Click away and it stops, and Enter is never pressed into the wrong
  place. It cannot see inside Claude, so it waits instead: for the chat to load, and for the
  uploads to finish (more time for bigger files). Both waits can be lengthened per connection.
- **Failures are not retried.** A hand-over that failed half way would paste twice if repeated, so
  the failure kind is "Not delivered", never retried. The share is kept as a zip like any failed
  share.
- **Capture folder.** Files that must exist on disk (the screenshot as shown, edited or not, and
  the ticked files, copied as they are when the button is pressed) live in a folder of their own
  under CargoFrame's data folder. It is made the first time a destination needs it, deleted when
  the capture window closes, and anything a crash leaves behind is swept at start-up and every
  hour. Only folders carrying CargoFrame's marker file are ever deleted.
- **Limits.** Claude takes at most 20 files (the screenshot is one) of 30 MB each in one message;
  CargoFrame says so before sending instead of letting the chat refuse.
- **Linux.** Needs X11 and `xdotool`; files cannot be pasted from CargoFrame there yet, so only
  the message goes, with a note. Wayland offers no way to press keys in another program.
- **Later (Phase 9).** Per-button rules for what goes to Claude, with a live "will send" line.

## Phase 8: the capture workspace (3 Nov 2026)

Items 8, 14, 15 and 17 of the owner's feedback of 2 Nov 2026.

- **Files are copied when the capture opens (item 8).** The files that are ticked in advance are
  copied into the capture folder in the background as soon as the window opens, so the share
  button has nothing left to do but send. A share that is pressed while the copying runs waits
  for it. What is sent is the files **as they were at capture time**; a zip kept after a failed
  send holds the same copies. Limits: 30 MB per file and 200 MB in total for the advance copies; a
  file above them is not copied ahead and is read from the project when the button is pressed,
  as before (so it is the one kind of file that can differ from capture time), and the usual
  zip limits then apply to it. The capture folder is still deleted when the window
  closes, and a start-up sweep (no age limit, since no window can exist yet) removes whatever a
  crash left; the hourly sweep keeps the two-minute grace for windows that are opening.
- **"Look in" for the Files block (item 17).** A Files block looks in any mix of: the project
  folder (the default, unchanged), the program's folder, and one, two or three folders above the
  program, each optionally narrowed to a named subfolder such as `logs`. A subfolder must be a
  plain relative folder: absolute paths, drive letters and `..` are refused when the layout is
  saved. A folder that does not exist (a program only two levels deep has no "three above") is
  skipped, not an error. The builder shows how many files each choice finds, using the folders of
  the last capture.
- **One root for file names.** Every file path shown, ticked or sent is relative to the highest
  folder any Files block of the layout looks in (the project folder when none looks higher).
  Two folders can hold a file of the same relative name only if they are different folders, so
  names do not collide; each read still goes through the same "stays inside the root" check as
  before.
- **A text block can name the zip (item 15).** Role *Use as zip name*: variables are filled in,
  characters Windows or Linux would refuse are removed, `.zip` is added, the stem is cut at 80
  characters, and an empty result falls back to the default name. A zip never overwrites an
  existing one (a number is added). A layout may have **one** such block; a second is reported
  when the layout is saved.
- **Data rules can read any file (item 14).** A new format, *Any file*, gives the name, size
  (`size`, `size: human`), modified time (`modified`, `modified: date`), line count and SHA-256
  fingerprint (`sha256`, `sha256: short`) of any file, text or not. Text search gains
  `between: a | b`. *If there are several values* gains **the one at a number** (1 is the first,
  -1 the last), and a regular expression can **build its value from groups** (`$1.$2`, `${name}`,
  `$0`); naming a group the pattern does not have is an error, not an empty value. Reading text
  out of PDF and Office files stays out until someone needs it.
- **The Rules page tries the rule by itself.** The Test button is gone: 0.4 s after the last
  change the rule runs on a real file of the chosen project folder (or of the last capture) and
  shows the value or the problem, the file that was read, and its first lines. A late answer for
  an older version of the rule is dropped.

## Phase 9: what goes to Claude, and what each button can send (3 Nov 2026)

Items 1, 3 and 9 of the owner's feedback of 2 Nov 2026, and the rule from the 3 Nov test: one
click on the share button sends everything captured, with no separate "copy prompt" step.

- **One capability table per destination (item 9).** `core/layout/src/capabilities.rs` says, for
  each kind of connection, whether it can take *text*, *dropdown choices*, the *screenshot*,
  *files* and *links in text*: yes, with a catch, or no, each with one line in plain words. The
  builder, the capture window and the share itself all read this table, so they cannot disagree.
  Jira takes everything. Google takes text and links as plain text only, with one list (the Drive
  folder). Reddit takes no files. Claude on Windows takes text, picture and files (pasted into the
  chat); elsewhere only the message goes, and text files can still travel inside it. A Custom API
  can carry only what its request describes: a body for text, a multipart field for the picture
  and one for the zip, lists for dropdowns. Saving a zip on this computer keeps everything.
- **A verdict for every block of every button.** `core/layout/src/reach.rs` gives each block one of:
  *sent* (as the screenshot, the files, the prompt, the zip name, or in a named field),
  *cannot be sent* (with the reason from the table), *switched off* (by the button's send rules)
  or *not used* (nothing maps it). The interface asks the Rust code (`button_reach`) instead of
  working it out itself. The in-memory stand-in used by tests (`ui/src/lib/demoReach.ts`) copies
  the table only so the interface can be built without the app.
- **The capture window says it, block by block.** Under every block, one small pill per button in
  that button's colour: ✓ with the button's name when it sends the block, ✕ with the reason when
  it cannot. A block no button sends gets one plain "Not sent". Hovering or focusing a button
  rings the blocks it sends. The "This button does not send …" warning is shown there and no
  longer repeated in the header.
- **A live line beside the button (item 1).** "Will send: Screenshot (edited) · 3 files (12 KB) ·
  Prompt · Note", worked out from the same verdicts and the values in the window. Empty text and
  files nobody ticked are left out, as the share leaves them out. For a Claude button the line
  above it says "Opening: <chat name>" (or "a new chat"), and the result says "Opened: <chat
  name>" after a successful share, so the person sees where it went before and after.
- **Colours.** Jira blue, Google green, Reddit orange, Claude brown, Custom API violet, "this
  computer" grey. White text on each fill stays above 5:1; the outline is darker in the light theme
  and lighter in the dark one so the button reads on the page either way. Colour is never the only
  signal: every button says where it sends in its tooltip, and every mark carries a ✓ or ✕ and the
  button's name.
- **Send rules on the button.** Per button: send the screenshot (on or off), and what happens to
  the ticked files: *attach*, *write text files into the message* (Claude only), or *leave out*.
  Stored with the button as `send`, left out of the file when it is all default. The inline
  limit is 64 KB per file by default, 1 to 512 KB; a file that is bigger, or not plain text, is
  attached instead and the result says how many. A rule the destination cannot honour is greyed
  out in the builder with the reason (no picture to a request without a multipart field, no
  files to Reddit).
- **Text files inside the message (Claude).** The message holds the prompt, then one "Name: value"
  line per mapped field, then each inline file as `File name:` and a fenced block. The fence is
  longer than any run of backticks in the file, so a file that contains a fence cannot close it.
  The note under the result says how many files were written in and how many stayed attached.
- **The chat is bound without a mapping.** A layout with a Claude chat dropdown and a Claude
  button no longer needs a hand-made `chat` mapping: the button uses the dropdown. Picking a chat
  opens that chat. (The link itself, spike S8, was answered by the owner's test of 3 Nov; the
  fix lives in the one-click branch.)
- **The builder.** Select a sharing button: the panel lists what its destination can take (✓, ◐
  with a catch, ✕), what the button does with each block of the layout, and the send rules. Select
  any other block: a "Sent by" list says which buttons send it. The canvas tags every block with
  ✓ Sent, ✕ Cannot send, – Left out or – Not used for the selected button (or the only button),
  and greys the ones that cannot go. A button with no active connection is drawn in the colour
  of "this computer", as it will save a zip there.
- **Still open for the owner:** the Windows-only parts (pasting the picture and files into the
  Claude app) and the real-account runs are not exercised from the build machine; the owner's PC
  test is what closes Phase 9, as the plan says.

## Phase 10: updates (3 Nov 2026)

Item 10 of the owner's feedback: "Check for updates" asks the CargoFrame repository for the newest
release, downloads the installer, checks it, installs it and restarts.

- **Our own small updater, not the Tauri updater plugin.** The plugin needs a signing key pair
  and a signed manifest published with every release, and cannot talk to a private repository
  without a proxy in between. The release workflow already publishes the installers and
  `SHA256SUMS.txt`, so the updater uses those. `core/update` is a crate of its own with no
  Tauri in it (check, download, verify, install plan), so every step is tested without a
  network or a real installer.
- **The check.** `GET /repos/SotharL/CargoFrame/releases/latest` on `api.github.com`. Drafts and
  pre-releases are ignored, tags like `v0.2.0` are compared as versions (newer only), and the
  installer for this computer is found by name: `CargoFrame-<version>-windows-x64-setup.exe` or
  `CargoFrame-<version>-linux-x64.tar.gz`. A release without that installer, or without
  `SHA256SUMS.txt`, is "not installable" and says why.
- **The token (private repository).** Pasted in Settings, General, Updates and kept in the system
  credential store under `updates/github-token`. It is write-only from the window: the section
  only learns *whether* one is set. It goes to `api.github.com` only; the download uses the API's
  asset address with `Accept: application/octet-stream`, and the redirect to the file host drops
  the sign-in header. Public repository later: remove the token, nothing else changes.
- **The checksum is not optional.** After the download the SHA-256 is compared with the line for
  that file in `SHA256SUMS.txt`. A missing file, a missing line or a different value means the
  installer is deleted and nothing runs. There is no "install anyway".
- **Installing.** Windows: the NSIS installer runs silently (`/S /UPDATE /R`), asks the running
  CargoFrame to quit (`--quit`), keeps the settings and starts CargoFrame again. Linux: the
  archive is unpacked and `install.sh --restart` runs. If the installer does not take over within
  90 seconds, or fails to start, the section says so and shows where the checked installer is
  kept, so it can be run by hand. Downloads left by an earlier run are removed at the next start.
- **When it runs.** Only when the button "Download and install" is pressed. The daily check
  (a minute after start, then every 24 hours) is **off by default**, switched in the same section
  ("Look for a newer version once a day"); it never downloads or installs, and shows one
  notification per newer version.
- **Done when (from the plan).** Installing a newer version over an older one keeps settings,
  tried on the owner's PC. The Windows install and restart cannot be exercised on the build
  machine; the flags, the quit handshake and the plan are covered by tests, and the real run is
  the owner's check.

## Phase 11: chat connectors, Slack first (3 Nov 2026)

Item 18 of the owner's feedback: Slack, Discord and Teams. Slack is built first; Discord and Teams
follow the same shape (a connector, a row in the capability table, a template, a form).

- **A bot token you create, not a sign-in.** Following the plan's decision, Slack uses a Slack app
  in the person's own workspace: they paste the Bot User OAuth Token (`xoxb-`) once, it is kept
  in the credential store under `<connection>/token`, and the Connections form shows the five
  steps to get it (create the app, add four scopes, install, paste, invite the app to the channel).
  The scopes are `chat:write`, `files:write`, `channels:read` and `groups:read`; `chat:write.public`
  is optional and lets the app post in public channels it has not joined. Nothing is registered
  by CargoFrame, so there is no shared client secret to protect.
- **What a button can fill.** `channel` (a list of the channels the app can see, grouped public
  and private; a typed `#name` is looked up), `thread` (a message link from "Copy link", or the
  timestamp), `message` (defaults to the prompt, then the window title) and `title` (an optional
  bold first line). A thread link also names its channel, so a button may map only the thread; a
  link into a different channel than the one chosen is refused before anything is sent.
- **Files.** The screenshot and the zip of the ticked files go through Slack's current upload
  steps (ask for an address, send the bytes, share them with the message as their comment); the
  old `files.upload` is gone. With no files the message is a plain `chat.postMessage`. Each step
  is recorded, so a retry after a failed share never uploads a file twice or posts twice.
- **Text is escaped.** `&`, `<` and `>` become entities, so captured text cannot write a Slack
  command such as `<!channel>`. The cost: a person cannot write `<https://…|word>` links by hand.
  Addresses become links in Slack by themselves.
- **Failures in words.** Slack answers with HTTP 200 and `ok: false`; each code that matters
  (`invalid_auth`, `missing_scope` with the scope it needs, `channel_not_found`, `not_in_channel`,
  `is_archived`, `msg_too_long`, rate limits, Slack's own errors) is turned into a sentence and a
  failure class: a revoked token asks to reconnect, a rate limit waits, a server problem retries.
- **Colour and template.** Berry (`--kind-slack`), apart from Jira blue, Google green, Reddit
  orange, Claude brown and Custom violet. "Slack message" is a ready-made layout: screenshot,
  channel, message, optional thread link, files, a Post to Slack button.
- **Discord (same shape, a bot token).** The person creates a bot in the Developer Portal, adds it
  to their server with an invite link made from View Channels, Send Messages, Send Messages in
  Threads, Attach Files and Create Public Threads, and pastes the token (`Authorization: Bot …`).
  No privileged intents are needed: only the REST API is used, so nothing keeps a connection open.
  Fields: `channel` (text channels and forums, listed by server; `#name` is looked up, and refused
  if two servers share the name), `thread` (an active thread or forum post from a list, or a link
  from Copy Link; it overrides the channel), `message` and `title`. In a text channel the title is
  a bold first line; in a forum it is the post's name (the window title when empty).
- **Nobody is pinged.** Every Discord message says `allowed_mentions: {parse: []}`, so
  `@everyone` in a window title or a data rule stays text. The message is cut to Discord's 2000
  characters, and the result says so.
- **Retries cannot post twice.** A channel message carries a `nonce` with `enforce_nonce`: if the
  answer was lost and the share is tried again, Discord returns the first message. A forum post
  has no such protection, so, like a Reddit post, it is not retried when its outcome is unknown
  (a lost answer or a Discord server error): the result says to look in the forum.
- **Failures in words.** 401 asks for a new token; 403 is *not* treated as an expired token (on
  Discord it means a missing permission) and says which permissions to give; "too large" says the
  server's upload limit; 429 waits for the time in the header or in the answer.
- **Test connection** says who the bot is and how many servers it is in, or that it is in none
  yet, which is the usual first mistake.
- **Colour.** Discord purple `--kind-discord`, next to Slack's berry and Custom's periwinkle.
- **Teams (a Microsoft sign-in, posted as the person).** Following the plan's decision, the
  person registers an app in their own Microsoft Entra ID (free): a "Public client/native" app
  with the redirect `http://localhost`, and six delegated Graph permissions (`Chat.ReadBasic`,
  `ChatMessage.Send`, `ChannelMessage.Send`, `Team.ReadBasic.All`, `Channel.ReadBasic.All`,
  `User.Read`). They paste the Application (client) ID; there is no secret. The sign-in is the
  shared OAuth code (browser, PKCE, a one-time server on this computer) and the refresh token
  stays in the credential store (`offline_access`). Work and school accounts only: Graph does not
  let a personal account post to Teams this way. Some organisations want an administrator to
  approve the permissions; the browser page says so and the sign-in error repeats the first line
  of what Microsoft said.
- **`http://localhost`, not `127.0.0.1`.** Microsoft ignores the port of a `localhost` redirect
  but not of `127.0.0.1`, and its portal does not accept `http://127.0.0.1` at all. The shared
  OAuth code got a second spelling for this (`Redirect::Localhost`: `http://localhost:<port>`,
  answered at `/`, and a request without a code, a state or an error is not taken for the
  answer). The one-time server still listens on `127.0.0.1` only. Google and Reddit keep their
  spelling.
- **What a button can fill.** `target` (one list: the channels of the teams the person joined,
  grouped by team, then their chats grouped as direct, group and meeting chats; values are
  `channel:<team>/<channel>` and `chat:<id>`), `thread` (a "Copy link" address of a channel
  message; it names the team and channel by itself, and a different channel than `target` is
  refused), `message` and `title`. In a new channel post the title is the post's headline; in a
  chat or a reply it is a bold first line.
- **The screenshot is inside the message** (`hostedContents`). **Ticked files are not sent:** a
  Teams file is a OneDrive or SharePoint file, which needs another permission and an upload
  session. The capability table says "no" for files, the template has no files block, and a share
  that still carries a zip sends the rest and notes that the files were left out.
- **Text is escaped** (`& < > " '`) and put in `<p>` and `<br>`: a window title cannot write
  markup or an `<at>` mention.
- **A post is not retried when its outcome is unknown.** A Graph message has no key that makes a
  repeat harmless, so a lost answer or a Graph server error ends the share with "look in Teams";
  a 429 waits for `Retry-After` and tries again, because nothing was posted. Shared channels
  (from another organisation) are left out of the list.
- **The token only goes to Graph.** A "next page" link in a list that is not on the Graph address
  is not followed. Lists read at most three pages of chats (150) and forty teams.
- **Colour.** Teams blue `--kind-teams`, darker than Jira's.
- **Still to do for this phase.** "Each posts to a test workspace" on the owner's side. Slack,
  Discord and Teams have only run against mock servers. Things only a real tenant can settle: that
  Microsoft accepts the `http://localhost` redirect with the answer at `/`, that chats come back
  in a useful order (the list is sorted by name, because combining `$expand=members` with
  `$orderby` is not something the mock can prove), the largest screenshot a message takes, and
  whether the organisation's policies allow posting with these permissions.

## Phase 12: video, the parts that do not wait for spike S9 (3 Nov 2026)

Item 16 of the owner's feedback. The plan says to answer spike S9 (capture and encoding) first.
S9 needs real Windows and Linux desktops, so what could be built and proved here was built:
everything after the recorded file exists, and a recorder that works on a virtual X screen.

- **The recorder is ffmpeg, run as a child process.** It captures, encodes and writes the file;
  no picture passes through CargoFrame, so the app keeps `unsafe_code = "forbid"` and no capture
  API of any platform is called from Rust. ffmpeg is found next to the app or on the search path
  (`locate`); whether to ship it or ask the person to install it is an S9 question (recommended
  until measured: ask the person to install it, with a setting for its location).
- **The rolling buffer is a ring of one-second files, not frames in memory.** Raw frames cannot be
  kept (10 s of 1080p is 2.5 GB, of 4K 10 GB). ffmpeg writes MPEG-TS segments and a list of the
  finished ones (`-f segment`); `Ring::poll` reads the list into a `RollingBuffer`, which drops
  segments older than the window (10 s by default, 1 to 30) or over the disk budget (400 MB by
  default) and deletes their files. A recording can run for an hour and hold a few MB. A segment
  has to start on a keyframe, so the recorder asks for one every second. The clip is the window
  plus up to one segment; the editor starts the trim at the last `window` seconds.
- **Stopping saves the last second.** `Recorder::stop` writes `q` to ffmpeg's input, so it
  finishes its segment and lists it; if it has not stopped after the patience given it is killed
  and the finished segments are still used. If it dies by itself, `poll` returns its last message.
  `cancel` stops it and removes the files. Joining the segments into one MP4 is a stream copy.
- **The edit model.** Trim (start and end), one crop, and up to 50 marks. Times are on the
  uncut clip's timeline, so cutting the start never moves a mark. The crop is rounded to even
  numbers (H.264 needs them), a crop of the whole picture is no crop, a clip must stay at least
  0.5 s, a mark only in the cut part is dropped and counted, and every problem is reported in
  words, not just the first.
- **Marks are pictures the editor draws.** The editor window draws a red circle or a text on a
  transparent PNG and says where and when it shows; ffmpeg lays it over the clip (`overlay` with
  `enable='between(t,…)'`). What the person saw in the editor is what is exported, and the core
  needs no fonts or drawing code. Marks go on the uncropped picture and the crop comes last, so
  moving the crop never moves a mark.
- **Export** is H.264 in an MP4 with the index first (it plays while it downloads), `veryfast`
  at CRF 23, no sound (CargoFrame records the screen only). The run can be cancelled and has a
  time limit; a failure returns ffmpeg's last line. Every path is given as `file:<path>`, so a
  mark picture named `concat:…` or `http://…`, or a path that starts with `-`, cannot become a
  protocol or an option.
- **Capture and encoder choices are the ones S9 measures:** Linux X11 `x11grab`; Windows
  `gdigrab` and `ddagrab`; encoders x264 (ultrafast/veryfast), NVENC, QSV, AMF, Media Foundation
  and VAAPI. Frames larger than 1080 lines are scaled down before encoding by default. Which is
  the default per machine is left to S9 (`docs/spikes.md`), with `scripts/windows/s9-video.ps1`
  and `scripts/e2e/s9-video-linux.sh` to measure it.
- **Not built yet:** the video block and the "video layout", the recorder window (start, stop,
  the editor with its timeline, crop and marks), per-connection sending of a video (size limits
  differ: Discord 10 to 25 MB, Slack more, Teams needs OneDrive), Windows and Wayland capture
  results from S9, and the shortcut that starts a recording. Wayland needs the portal's PipeWire stream, which ffmpeg reads only from 7.1; S9
  says whether that route or GStreamer is needed.

### Phase 12, part 2: the Video settings page, and a test recording you can run (3 Nov 2026)

Spike S9 needs a person at a real Windows desktop, so the app now lets that person run the
important part of it without a script: Settings, Video.

- **ffmpeg is the person's to install, with a setting for where it is.** The page takes a folder
  (or the ffmpeg program itself); when a place is given it is the only place looked in, so a wrong
  folder says so instead of quietly using another ffmpeg. Empty means next to CargoFrame, then the
  search path. The message when it is missing gives `winget install Gyan.FFmpeg`. Shipping
  ffmpeg with the installer stays an S9 and licence question (recommended until measured: not).
- **"Test ffmpeg" reads what this ffmpeg has** (`-encoders`, `-devices`, `-filters`: gdigrab,
  ddagrab, x11grab; x264, NVENC, QSV, AMF, Media Foundation, VAAPI) and shows what *Automatic*
  would use. A choice this ffmpeg lacks is greyed out. An encoder being listed does not mean this
  PC can use it (no graphics card, no driver), so the page says that, and the test decides.
- **Automatic is the safest choice, not the fastest.** Windows: GDI capture (works in remote
  sessions and virtual machines, where Desktop Duplication may not) and software x264
  `ultrafast`; a build without x264 falls back to the first hardware encoder it has. Linux: x11grab
  on an X11 session, the whole screen. A Wayland session gets a plain "not supported yet"
  rather than a black recording. S9's numbers decide whether Automatic should change.
- **"Try a 5-second recording"** records with the settings in the form (saved or not), joins the
  pieces, reads the clip back with ffprobe and reports size, length, time taken and the capture
  and encoder used. A failure shows ffmpeg's last line plus a hint that fits what was tried
  (hardware encoder: choose Software; Desktop Duplication: try GDI). The clip is kept in
  `video-test/` (replaced by the next test) so the person can open it and see that it shows the
  screen; the pieces are deleted. Only one test runs at a time.
- **No console window flashes up.** CargoFrame has no console, so every ffmpeg it starts on
  Windows is started with `CREATE_NO_WINDOW`.
- **Settings:** `video.json` with capture, encoder, frames per second (5 to 60, default 30),
  seconds kept (1 to 30, default 10), and the largest height (0 or 240 to 4320, default 1080).
  Out-of-range values are refused with a sentence; the file tolerates missing and unknown fields.
- **Not verified:** the Windows paths (GDI, Desktop Duplication, the hardware encoders, the
  hidden console) have not run on a Windows machine. The Linux X11 path ran against a virtual
  screen in the tests.

### Phase 12, part 3: record until stopped, and never leave ffmpeg running (3 Nov 2026)

- **One recording at a time, owned by one service** (`Recording` in `app/src/video.rs`): idle,
  recording, finishing, or failed-with-a-reason. Settings, Video, "Record until I stop" starts it,
  the page watches it twice a second (time so far, how much is kept, whether the disk limit cut the
  start) and "Stop and make the clip" joins the pieces and reports the clip like the 5-second test.
  A long recording keeps only the last N seconds, so what the owner watches in Task Manager is what
  a real recording will cost. The trigger that will start the same service is not built yet.
- **ffmpeg can never be left capturing.** Dropping a `Recorder` that is still running kills ffmpeg
  and removes its pieces; CargoFrame quitting (`RunEvent::Exit`) and the Settings window closing
  cancel a running recording; and ffmpeg is given `-t 3600`, so even if CargoFrame is killed the
  leftovers stop growing after an hour. A recording started by hand stops itself after 10 minutes
  (checked while the page is open). A crash can still leave an `ffmpeg` process for up to an hour
  and its pieces in `video-test/ring`; the next start of a test replaces the folder.
- **A failure while recording is reported once, in words,** with the same hint as a failed test
  (hardware encoder: choose Software; Desktop Duplication: try GDI).

### Phase 12, part 4: can the window play the clip? (3 Nov 2026)

The editor will show the clip in the window, so whether the web view can play H.264 is a question to
answer before building it. Settings, Video, "Watch it here" sends the test clip to the page as bytes,
the page makes a blob URL and plays it in a `<video>`; if the web view cannot, it says so and points
to the video player. Expected: WebView2 on Windows plays H.264; WebKitGTK on Linux needs the
GStreamer H.264 plugins (`gst-libav` / `gst-plugins-bad`) and may not. If it cannot play it, the
editor falls back to scrubbing through still frames that ffmpeg extracts, and that is a decision for
after the owner has tried it.

- **The content security policy gains one directive, `media-src 'self' blob:`.** Nothing else
  changes: still no remote source of any kind, and a test pins the directive. The clip is sent as raw
  bytes (not JSON numbers), at most 100 MB, only by the Settings window's own permission; the CargoFrame
  window is not granted it (a test lists the video commands it must not hold).

### Phase 12, part 5: export a trimmed, cropped, marked copy of the test clip (3 Nov 2026)

Settings, Video, under a test clip: "Trim, crop and mark a copy" makes an edited copy
(`video-test/exported.mp4`) the way an edited video will be saved, so the owner can check ffmpeg's
export on Windows before the editor exists.

- **It is the real export path.** The command builds an `Edit` (start, end, crop, one mark) and calls
  the same `plan` and `run` the product will use. The start and end are seconds in the form and
  milliseconds on the wire; a crop is rounded to even sides; a cut shorter than half a second, or a
  crop outside the picture, is refused with the same sentence the product will show.
- **The test mark is drawn by ffmpeg itself** (a translucent red square from a `color` source), so
  the page needs no drawing code yet. It exercises the overlay filter graph with its time window.
  The editor will send marks it draws on a canvas instead.
- **Only the test clip, only the Settings window.** The command takes no path: it reads
  `video-test/test-clip.mp4` and writes `exported.mp4` in the same folder, so the page cannot make
  ffmpeg read or write anywhere else. It refuses while a recording is running.
- **Not built yet:** the editor itself (timeline, drawing crop and marks over the picture), the
  video block and layout, sending a video, and the shortcut that starts a recording.
- **More than one screen.** GDI (`gdigrab desktop`) records every screen as one wide picture, then scales it
  to the largest height; Desktop Duplication (`ddagrab`) records output 0, the first screen Windows lists
  (usually, not always, the main one). Choosing the screen to record is part of the trigger work that is not
  built yet, and S9 should say what each does on a two-screen PC.

### Phase 12, part 6: marks drawn by the window itself (3 Nov 2026)

The editor's marks (red circle, text) are drawn by the window on a canvas and laid over the video by
ffmpeg. This slice proves that path before the editor is built: "Mark" in the copy panel can be
*A circle and a text (drawn by this window)*.

- **The window draws a see-through PNG the size of the clip** (a circle in the middle, a text with a white
  outline at the top left, in a system font) and sends it as raw bytes (`set_video_test_mark`; checked to
  be a PNG, at most the size limit of an edited screenshot, written to `video-test/mark-drawn.png` only).
  The export then lays it at 0,0 with the same overlay filter graph, cut and crop included (the crop
  comes last, so a crop never moves a mark).
- **Checked here in a real Chromium:** the picture the page draws (1920×1080, about 86 KB) composited by
  ffmpeg over a test picture with the circle and the text placed correctly and the see-through parts
  left clear. A mark that covers the whole picture is cheap enough (tens of KB); the editor will send one
  picture per mark, sized to the mark, to keep it smaller.
- **Windows fonts are the open question:** the text uses `bold <size>px sans-serif`. The window's own font
  is drawn, so the owner should look at how the text comes out on Windows (S9 checklist).

### Phase 12, part 7: editing on the picture (3 Nov 2026)

First editor controls, inside the Video page, on the test clip: while the clip plays in the window
(after "Watch it here"), "Start here" and "End here" take the trim from where it is playing (to a tenth
of a second), and while "Crop to an area" is ticked a box is drawn on the picture with everything outside
dimmed. Drag the box to move it, its corner to resize it.

- **The crop is always in the clip's own pixels, on even numbers, inside the picture and at least 16
  pixels each way,** whatever size the clip is shown at; the numbers in the fields stay in step, and
  typing in them moves the box. The fields remain the way to do it from the keyboard.
- **Mouse events, not pointer events,** because CargoFrame is a desktop app and the tests (jsdom) can
  drive them. A touch screen is not a V1 target.
- **Checked in a real Chromium** with a real clip: the box sits where the numbers say, dims the rest, and
  follows a drag. The box and the buttons are only on the test clip, not on an exported copy.
- **Still to build for the editor:** the editor in the CargoFrame window instead of Settings, with more
  than one mark on a clip (the timeline strip is part 9, the click-to-place mark is part 8).


### Phase 12, part 8: click to place the drawn mark (3 Nov 2026)

When "Mark" is set to the window-drawn one, clicking on the picture puts the mark there: a small dot
shows the spot, and a "Mark text" field sets the words written under the circle (empty gives only the
circle). It starts in the middle of the clip.

- **The position is kept as a share of the picture (0 to 1),** so it stays right whatever size the clip
  is shown at, and is turned into the clip's own pixels only when the mark is drawn. The circle's size is
  a tenth of the shorter side, so it reads the same on a small or a large clip.
- **A click does nothing unless a drawn mark is chosen, and while "Crop to an area" is ticked,** because
  the same picture is then used to drag the crop box. Both cases are in the tests.
- **Still to check on a real PC:** that the text is drawn with a readable font on Windows (the drawing
  uses the window's own fonts, so it is part of the owner checklist in `docs/status.md`).

### Phase 12, part 9: a timeline under the picture (3 Nov 2026)

Under the playing test clip there is now a strip that stands for the whole clip: the part that will be
kept is shaded, a thin line follows the playing position, and the two ends have handles. Drag a handle to
move the start or the end; click on the strip to take the clip to that moment. The two number fields stay
and move with it, and "Start here" / "End here" still work.

- **The trim is always in tenths of a second, and at least a tenth of a second is kept,** so the start
  cannot pass the end and the end cannot come back below the start.
- **From the keyboard:** each handle is a slider; the arrow keys move it a tenth of a second, with Shift
  a second. Grabbing a handle never moves the clip (the click is not passed on to the strip).
- **Checked in a real Chromium** with a real clip: the strip fills the width, a drag moves the handles
  and the numbers, and a click on the strip seeks. Not shown on the edited copy, only on the test clip.

### Phase 12, part 10: the video block, and a trigger that makes a video (3 Nov 2026)

The data side of "a trigger that records". Nothing records yet from a trigger (that is the next part);
this part is what layouts, triggers and the sending table need to say first.

- **A trigger has a `media`: `screenshot` (the default, and what every trigger file written before
  this means) or `video`.** The Triggers page has a "Makes" choice; for a video the "Capture" choice (window,
  screen, all screens) is hidden, because the recorder follows Settings, Video. A card warns when its layout
  does not fit (a video trigger with no Video block, a screenshot trigger whose layout has only a Video block).
- **A layout has a `video` block, next to the screenshot block.** A trigger makes a screenshot or a video, so a layout
  with both gets a warning ("keep the video block in a layout of its own"); it is not an error, because
  both blocks can be there while someone is changing a layout.
- **The send rules have no new switch:** the button's "Send the screenshot" switch is the "send the capture"
  switch, and reads "Send the video" in a layout with a video block.
- **The capability table has a `video` row.** Recommended answers, from what each connector already does with
  an attachment: Slack yes; Google (Drive) yes; Jira and Discord with a catch (their attachment size limits);
  Reddit, Teams, Claude and Custom API no, each with the reason. Saving a zip keeps it as `recording.mp4`.
  These are claims about what the connectors can carry; the sending itself is part of the next parts, and
  the real posts to Slack, Discord and Jira stay on the owner's list.

### Phase 12, part 11: a trigger that records, and a capture window that plays the clip (3 Nov 2026)

A video trigger now does what its name says. These are the choices; the recommended answer was taken wherever
there was one.

- **The first press starts recording, the next press stops it** and opens the CargoFrame window with the clip.
  The shortcut is the same both times, so there is nothing to remember; the notice that appears on the first press
  says how many seconds are kept ("only the last 30 seconds").
- **A recording started by a trigger has no ten-minute limit,** because it is stopped by hand; the 30-second
  rolling buffer in Settings still decides what is kept. A recording started from Settings keeps its ten minutes.
- **The tray has "Stop recording and discard"** for when the person starts by mistake: nothing is opened, the clip is deleted.
- **A second press while a capture is open or being prepared does nothing** (as for a screenshot trigger);
  a recording that stopped by itself is reported once, and the next press starts again.
- **The clip is moved into the capture's own temporary folder,** and goes with it when the window closes
  (`recording.mp4`, deleted on drop like the screenshot). Nothing stays in the work folder.
- **The window plays the clip in a Video block** with its length, size and weight. If the window's web view
  cannot play it (H.264 on some Linux systems), the block says so and the clip is still sent.
- **It is sent through the same attachment as the picture,** named `recording.mp4`; so Slack, Discord, Jira and
  Google Drive carry it with the code they already have. The button's "send the capture" switch decides.
- **A destination that cannot take a video is not sent one,** and the reason from the capability table is shown
  in the window and in the result (Reddit, Teams, Claude, Custom API).
- **A recording over 100 MB is refused before it is read,** with the size in the message. Discord's and Jira's own
  limits are not guessed: the service answers and its answer is shown.
- **Not in this part:** editing in the capture window (trim, crop, mark are in Settings only so far), a ready-made
  layout for video, per-connection size hints. They are the next parts.

### Phase 12, part 12: ready-made layouts for a recording (3 Nov 2026)

Four starting points in the layout builder for a trigger that makes a video: **Save a recording locally**,
**Jira bug report with a recording**, **Google Drive upload of a recording** and **Slack message with a recording**.

- **Each is built from the template for a picture,** with the picture block swapped for a Video block in the
  same place and size. The two cannot drift apart: a change to the Jira bug template shows in both.
- **Discord, Reddit, Teams, Claude and Custom API have none:** Reddit, Teams, Claude and Custom API cannot take
  a video, and Discord's upload limit (often 10 MB) makes a 30-second recording a poor default. Anyone can still
  put a Video block in a Discord layout; the window says what the server answers.
- **No new validation noise:** the four are clean for a signed-in connection and ready to send, like the others.

### Phase 12, part 13: cutting the recording in the CargoFrame window (3 Nov 2026)

The first editing the CargoFrame window offers on a recording: **keep only a part of it.** Crop and marks on a
recording are still in Settings, Video only.

- **A strip under the recording** (the same one as in Settings: kept part shaded, a handle for each end, a thin
  line for the playing position, a click moves the picture there) with "Start here", "End here" and "Keep it all".
  The handles also work from the keyboard (arrows a tenth of a second, Shift a second, Home and End).
- **Playing stays inside the kept part:** it stops at the end of it and starts again from its start.
- **The window does not cut anything while the person moves the handles.** The choice travels with the share
  request (`clip_edit`) and the cut is made once, just before sending or saving, into the capture's own folder
  (`recording-edited.mp4`, deleted with the capture). The original recording is never changed, so "Keep it all"
  always works, and a share that is retried does not cut twice.
- **A cut that keeps everything is not made,** and then ffmpeg is not needed at all. A cut that cannot be made
  (ffmpeg missing, a recording ffmpeg cannot read) stops the share with the reason, and nothing is sent or saved.
- **The line beside the button says what goes:** "Video (3.0 s of 9.5 s, cut)"; the size is not shown for a cut
  because it is not known before the cut is made. The 100 MB limit applies to what is sent, after the cut.
- **Tested** with a real ffmpeg (6 s clip, 2 s kept: the zip holds a 2 s video), and with fake recordings for the
  paths that need no ffmpeg. How the strip looks in the real window, and whether a given WebView plays the
  recording at all, are owner checks.

### Phase 12, part 14: cropping the recording in the CargoFrame window (3 Nov 2026)

A **Crop** tick next to the cut buttons puts a box over the recording (80% of the picture, centred).

- **Drag the box to move it, its corner to resize it;** from the keyboard, the arrows move it (Shift: a bigger
  step) and Alt with an arrow resizes it. What is outside the box is dimmed. The box cannot leave the picture and
  is never smaller than 16 pixels.
- **While cropping, the player's own controls give way to a Play/Pause button,** because the box covers the picture
  and would swallow the clicks. Cropping off brings the controls back.
- **Sizes are kept even in the window, and made even again by the program before cutting** (the encoder needs an even
  picture): a crop of 161×101 at 41,21 comes out as 160×100 at 40,20. This is tested with a real clip.
- **The window's edit is now one thing, `clip_edit` (start, end, crop),** replacing the first version's `clip_trim`.
  "Keep it all" undoes the cut and the crop together. A crop alone is enough to make a copy; nothing changes while
  the person drags, the copy is made once when sending or saving, as for the cut.
- **The line beside the button says it:** "Video (9.5 s, cropped)", "Video (3.0 s of 9.5 s, cut and cropped)".
- **Still only in Settings, Video:** marks (a drawn circle or text) on the recording.

### Cleanup after Phase 12 (3 Nov 2026)

A pass over what is in the repository and what is no longer used, done when the video work reached the window.
Nothing here changes what the program does.

- **Branches:** everything on the stacked branches was merged to `main` in pull request #19 (3 Nov 2026), and the
  local copies were deleted. Deleting remote branches is refused from the cloud session (GitHub answers 403). The
  older merged branches have since been removed on GitHub; the fifteen below were still there when this was written.
  All of them are inside `main`, so they can be deleted on GitHub (Branches page) or from the owner's own computer
  with this one command. Only `main` should remain.

  ```
  git push origin --delete chore/remove-unused-code feat/phase12-video-editor-controls feat/phase12-video-export feat/phase12-video-layout feat/phase12-video-mark-place feat/phase12-video-marks feat/phase12-video-preview feat/phase12-video-recording feat/phase12-video-settings feat/phase12-video-templates feat/phase12-video-timeline feat/phase12-video-trigger feat/phase12-video-window-crop feat/phase12-video-window-mark feat/phase12-video-window-trim
  ```
- **Code that nothing used:** `Frame::into_rgba`, `Frame::to_png_small`, `Layout::first_free_position` (the builder page
  places new blocks itself, in `model.ts`), the Tauri command `reload_triggers` and its permission (saving or deleting
  a trigger already reloads the shortcuts in the program), and the unused dependencies `tracing` in `cargoframe-deliver`
  and `@testing-library/jest-dom`.
- **Test-only helpers are test-only:** `find_tools`, `Multipart::with_boundary` and `ClaudeConnector::on_windows` are
  compiled only for tests. `Dirs::kept_zips` was already there and is now the one place that names the "kept" folder.
- **Kept on purpose:** `scripts/windows/try-claude.cmd` (the hand-run Claude delivery test, still on the owner's list),
  `scripts/dev/check.cmd` (the same checks as `check.sh`, for Windows), and every spike script.

### Phase 12, part 15: a mark on the recording in the CargoFrame window (3 Nov 2026)

A **Mark** tick puts a red ring on the recording, with an optional text under it. With this, what the window can do
to a recording matches what it can do to a screenshot: cut, crop and mark.

- **The ring starts in the middle of the picture; click the picture to move it,** or focus it and use the arrow keys
  (Shift: a bigger step). The text is typed in a box that appears next to the tick. One mark, in red: the same ring and
  text as the Settings test (a tenth of the shorter side in radius, the text just under it, outlined in white).
- **The window draws the mark itself on a see-through picture the size of the recording** and uploads it just before
  sharing (`set_clip_mark`, a command only the CargoFrame window has, named by the capture like the edited picture);
  the program then lays it over the kept part with ffmpeg, on the uncropped picture, so a crop and a mark
  together come out as they look in the window. What is shown in the window is drawn by the same numbers, so the two match.
- **The mark shows for the whole kept part.** Marks that appear and disappear at chosen moments are not in the window
  (the program can do it; the window has no place to choose the times yet).
- **While the crop or the mark is on, the player's own controls give way to a Play/Pause button,** because the boxes
  cover the picture. "Keep it all" undoes the cut, the crop and the mark together.
- **A mark that was not uploaded** (the window closed in between, say) stops the share with "The mark was not drawn:
  try again"; nothing is sent. The line beside the button says it: "Video (3.0 s of 9.5 s, cut, cropped and marked)".
- **Tested** with a real clip and a real overlay: a red square in the corner of the mark shows as red in the first picture
  of what is sent, and is not red in the recording itself. Whether the text comes out the same on Windows fonts is an
  owner check, as for the Settings test.

### Phase 12, part 16: small things that came up (3 Nov 2026)

- **The tray's tooltip says when the screen is being recorded** by a video trigger ("CargoFrame is recording your
  screen. Press the video shortcut again to stop."), next to the notice shown when it starts and the tray entry that
  throws the recording away. A recording started from Settings keeps its own indicator on that page.
- **The tests that record a real virtual screen run one at a time** (in the program's tests and in the video
  crate's). Under a busy computer two of them had failed once each with no message: each promises the recorder
  a couple of seconds, and a recorder starved of time returns no clip. They passed alone and in loops.

- **Two tests keep the commands in step.** One (in the program) checks that every command is registered, listed in
  the build script and given to at least one window, and that a window is never given a command that does not exist.
  It found a leftover permission for a command that was removed earlier (`allow-copy-text`), now deleted. The other
  (in the page) checks that every command the page calls exists and that none is defined but never called. The page
  test reads the Rust source as text; it declares the single Node function it needs instead of pulling the Node
  types into the whole page project.

### The logo, and an accent colour you can change (3 Nov 2026)

- **The logo is the "Convoy" mark** from the owner's files: three frames moving right, the lead frame carrying an
  arrow, with the two-frame small drawing below 48 px. The eight files are in `assets/brand/` as they were drawn,
  with one change the owner asked for: the colours are the development plan's design system, not the orange and navy
  of the first files. The frames are the text colour (`#1f1e1d` on light, `#f5f4ee` on dark) and the arrow is the
  accent (`#4f8ee0`). Nothing sits behind the mark.
- **The arrow is the accent colour, and the accent is a setting** (Settings, General, Appearance): suggested colours,
  a colour picker and a typed code, with a warning when a colour is hard to see (below 3:1) on the light or the dark
  theme, and a "Reset to blue". It is saved in `general.json` as `accent` (an older file without it gets the blue, a
  value that is not a colour counts as the blue). The text on an accent button turns white when the dark text would
  read worse. It colours both windows (a capture window kept ready between captures reads it again for each capture)
  and the arrow of the tray icon.
- **The logo in the app** is one component (`Logo.svelte`) that draws the mark with the theme's text colour and the
  accent as colours, so it follows both the theme and the accent. It is in the Settings sidebar. The capture window
  has no header, because its layout is the owner's own design.
- **Icon files** (`app/icons`) are drawn from the same artwork by `scripts/dev/render-icons.mjs`, transparent, with
  the frames in a mid-grey (`#8b8a84`, between the plan's two quiet-text colours) so they show on a light and on a
  dark taskbar, and the blue arrow. This answers the open question in the brand notes about surfaces that cannot
  switch theme. The tray icon is two layers (frames, arrow) joined in the program so that the arrow takes the
  accent; the executable, installer and window icons keep the default blue.
- **Not checked on a real Windows desktop:** how the icons look in the taskbar, the tray and the installer, at 100,
  125 and 150 percent scaling, and the colour picker in WebView2.

### The capture window, after the first real use (6 Nov 2026)

The owner tried the window and listed what was awkward. Each item and what was decided:

- **The screenshot toolbar is one line.** Icon buttons only (each with a name and a tooltip). The eight colours and
  the three thicknesses moved into one small button that shows the current colour and opens a panel; undo, redo,
  reset crop and clear all became icons. A block too narrow for that wraps rather than cutting anything off.
- **The video has the playback buttons people know**, laid over the top of the picture, see-through, shown while the
  pointer is over the picture or a button has focus: play/pause, stop (back to the start of the kept part), slower,
  faster (0.25, 0.5, 1, 1.5 and 2 times), mute and volume, and full screen. A click on the picture plays or pauses,
  a double click goes full screen, and Space, K, F, M and the arrow keys work when the picture has focus. The
  browser's own bar is not used, so the recording looks the same in WebView2 and WebKitGTK.
- **The timeline is a thin track the full width of the picture** with the two cut handles, the shaded kept part and
  the playing position. "Start here" and "End here" are gone: the handles do the cutting. (The Video page in Settings,
  which tests a recording, keeps its own buttons.)
- **Crop and mark take turns.** One of them has the mouse at a time. *Crop* shows the whole picture with a box that is
  already what will be sent; *Apply crop* leaves the mode and the window then shows only the cropped part (the
  recording is laid out so the box fills the picture, and the rest is clipped), *Cancel* puts back what was there
  before, and *Edit crop* and *Reset to full capture* are there afterwards. The mark is placed in the cropped picture;
  while the box is being set it stays in view but cannot be moved. Starting one tool while the other is open ends the
  other and keeps its result. *Reset edits* undoes the cut, the crop and the mark together.
- **Not checked on a real desktop:** full screen from a page inside WebView2 and WebKitGTK, and hover on a touch screen
  (there is none: the buttons are also reachable with the keyboard).

### Which button sends what: tick boxes (6 Nov 2026)

The owner found the line of pills under every block took too much room and wanted the choice to be something to
tick, colour-coded to the button, up to five side by side, safe, and easy to find. Decisions:

- **A small row of boxes on the top edge of every block, one per sharing button.** Each box has the button's
  destination colour and a ✓ when ticked, so colour is never the only sign; two buttons to the same kind of service
  get a small 1, 2. The row sits on the edge of the block, not under it, so no layout space is used; the Logs block's
  All/None moved next to its title to make room. The old pills ("✓ Jira", "✕ reason", "Not sent") are gone.
- **The boxes start as the layout says and are the choice for this capture.** A box is ticked at the start when the
  button's rules send the block. Clearing one leaves the block out of that button's share only; ticking a block the
  button had switched off (the picture, the files) sends it this once. Nothing is saved back to the layout.
- **A box that cannot be ticked is shown dashed and says why on hover**: the destination cannot take it (the reason
  from the capability table) or the button has no field for it. **A box the button cannot work without stays ticked
  and ringed**: the prompt of an AI chat and any field that the *Destination fields* page marks as required (the app
  tells the layout checks which fields those are).
- **The sharing code decides, not the window.** The window sends the ticked block ids with the share. The Rust side
  (`cargoframe_layout::select`) refuses a box for a block the destination cannot take, a block the button has no field
  for, a block that is not in the layout, and clearing a required one, with a sentence for each; it then drops the
  cleared blocks' fields, prompt, files and picture. A Custom API whose request uses a field that was cleared is
  refused by name ("The request uses {{summary}} but nothing fills it") and the share is kept, as before.
- **Hovering or focusing a button, or one of its boxes, outlines every block that button sends.** The outline is in
  the button's colour; the layout does not move.
- **The line under a sharing button is one short line** ("Sends: Screenshot · 3 files (12 KB) · Prompt → Release
  notes") with the whole text and the note in its tooltip. A button block only one row high hides the line and says
  the same in the button's tooltip.
- **At most five sharing buttons per window.** The builder disables "Add a sharing button" at five, says why, and the
  layout check marks a sixth button as unable to send, so a hand-edited file cannot get round it.
- **In the layout builder the lines for text and dropdown blocks are now choices** (not sent, the prompt, or one of
  the destination's fields), taken from the destination's own field list so a typo cannot happen. A field another block
  already fills is shown and cannot be taken twice; a field typed by hand in "Fill the service's fields" is kept in the
  list; Claude's chat dropdown shows that it is always used. The picture and the files keep their switches under "Send
  rules". A button that only saves a zip keeps plain lines.
- **Not done:** per-block choices for several Files blocks on one button (the switch covers them all), and changing the
  choice from the block's own "Sent by" list (it stays a plain list; change it on the button).

### Several windows, the window in focus, and sound (6 Nov 2026)

The owner asked for up to ten CargoFrame windows at once, for a video to be able to record just the window in focus,
and for the video to have sound (the computer's by default, no microphone), chosen in the layout. Decisions:

- **Up to ten capture windows, each its own capture.** Pressing a shortcut while another capture is open opens a new
  window instead of replacing it (the new one is placed a little below and right of the last, so they fan out). The
  eleventh press says so in a notification and does nothing. Each window has a label (`capture`, `capture-2` …) and its
  own session, its own ticked boxes, its own edited picture and mark, and its own capture folder; closing one forgets
  only that one, and "Send" in one window never touches another. Window events (`session-changed`) are sent to one
  window, and the capability file lets `capture-*` use the same commands as `capture`. Only one capture is *started*
  at a time (the screenshot or the recording stop is quick), but any number of finished ones stay open.
- **What a video records is set in the layout's Video block,** not in a global setting, because the same person has
  a "bug clip of this window" layout and a "tour of my screen" layout. The block has two settings: **Record** (*the
  whole screen*, or *the window in focus*) and the sounds (*sound of the computer*: on / off / one device;
  *microphone*: off / on / one device). New blocks and old layouts get the screen, the computer's sound on, and no
  microphone.
- **The window in focus is the one that has the focus when the first shortcut is pressed,** as it is placed and sized
  then (the visible frame on Windows, without the invisible borders). The recording does not follow it if it moves
  or is resized. If the system does not say which window it is (Wayland, nothing focused) or it is a stub smaller than
  16 pixels, the whole screen is recorded and the notification says so. A part of the desktop is cut out by ffmpeg's
  own capture (`gdigrab -offset_x/-offset_y/-video_size`, `x11grab` with `display+x,y`); on Windows, Desktop
  Duplication only sees one screen, so a window recording uses GDI (which sees the whole desktop, any screen).
- **Sound by backend.** Linux: PulseAudio (PipeWire answers the same way): the computer's sound is
  `@DEFAULT_MONITOR@`, the microphone is `default`, and the layout can name one device (the list comes from
  `ffmpeg -sources pulse`; a name ending `.monitor` is a computer sound, the others are microphones). Windows: ffmpeg
  has no way of its own to record the sound of the computer, only DirectShow devices, so the computer's sound is
  recorded when the PC has a loopback device (*Stereo Mix*, *What U Hear*, *virtual-audio-capturer*, found by name)
  and the notification says what to turn on when it has not; the microphone is the first DirectShow microphone. A
  native Windows loopback capture is a later step and is **not** built. _(Built in the second 7 November round: see
  "What the computer plays" below. Stereo Mix is no longer needed.)_
- **Sound never costs the picture.** No sound system on Linux (checked by looking for the PulseAudio socket, because
  asking ffmpeg takes seconds), a device that is not there, or any other reason ffmpeg gives up in the first
  moments: the recording goes on without sound and the notification says why. Two sounds are mixed into one track
  (`amix`), AAC at 128 kb/s, 48 kHz stereo; a third is ignored.
- **The clip keeps its sound through the export.** The probe says whether a recording has a sound track
  (`Clip.has_audio`); the export copies it (AAC), cut with the picture, and drops it only when there is none. The
  player shows the volume and mute buttons only for a clip with sound and says "No sound" otherwise.
- **Not checked here (no audio hardware, no real desktop):** the sound devices themselves (Pulse monitor and
  microphone, Windows loopback), a window recording of a real GPU window, DPI scaling with GDI on Windows (ffmpeg's
  GDI capture may take a scaled screen at the scaled size), and ten windows on a real desktop.

### A one-button installer, and a check for the programs CargoFrame uses (6 Nov 2026)

The owner could not build the app, does not want build tools (Rust, Node, Visual Studio) on the PC for security
reasons, and asked for the tool to check that the programs it needs are installed, and for an installer that is a
single button. Decisions (taken for them, the safest option each time, while they were away):

- **Nobody has to build anything.** CI builds the Windows installer; a version tag makes a draft release with one
  Windows file, `CargoFrame-Setup.exe`, and the Linux package. The README's Windows steps start from that download.
  Building from source stays documented, for developers only.
- **The installer is one page with one button.** It says what will happen, and its button reads **Install** (or
  **Update** over an existing copy). There is no welcome, folder, shortcut or options page and no finish page; the
  progress bar closes by itself and the installer opens CargoFrame. It is still per user (no administrator password,
  everything under `%LocalAppData%` and `HKCU`). "Run CargoFrame when I sign in" is on, because CargoFrame is a tray
  program that only works while it runs; an upgrade keeps an earlier "off", Settings, General changes it, and
  `/NOSTARTUP` leaves it off for a silent install. The window installer makes no desktop shortcut (the Start menu has one; silent installs keep making one).
  `installers/windows/click-test.ps1` opens the real window on a Windows runner, presses the button and checks the
  result, with screenshots.
- **What CargoFrame itself needs is nothing but Windows 11** (the web view, WebView2, is part of it; the installer
  fetches it from Microsoft only if it is missing). **ffmpeg is the one optional program** (video); on Linux also
  `xdotool` (pasting into a Claude chat, X11) and a sound system (PulseAudio or PipeWire).
- **Settings, Setup** checks them and shows what each is for. The number of missing programs is on the tab. The
  first time CargoFrame is started by hand it opens on this page (a marker file `setup-seen` in the settings folder
  makes it once only). On Windows, one button, **Install ffmpeg**, runs
  `winget install --id Gyan.FFmpeg --exact --silent --accept-package-agreements --accept-source-agreements
  --disable-interactivity`: the arguments are fixed in the program (no text from the screen or a file reaches a
  command), the package is Windows' own catalogue entry installed per user, nothing runs as administrator, it has a 15
  minute limit, only one install runs at a time, and a failure shows winget's own message with a "Try again" button.
  When winget is missing, the page says to get "App Installer" from the Microsoft Store or to choose an ffmpeg folder
  under Video. On Linux the program never installs anything: it shows the command for the distribution (`apt`, `dnf`
  or a generic line) with a copy button, because installing needs the user's password.
- **Why winget and not a download of our own.** A download we do ourselves would mean choosing a build of ffmpeg,
  checking it ourselves and keeping the link alive; winget does that with Microsoft's catalogue, its hash check and
  its own record of what is installed, and removes it with the usual "Uninstall".
- **Computer sound on Windows** _(superseded: CargoFrame now listens to the output itself, see "What the computer
  plays" below)_. The Setup page names the default output and has a button that opens the Windows sound settings.
- **ffmpeg is found without a new PATH.** After winget installs it, the app looks in winget's links folder as well,
  so no sign-out is needed; if it still cannot find it, the message says what to do.
- **A release can be made without pushing a tag.** The release workflow can be run by hand with the tag left
  empty: it builds the tip of the chosen branch, runs the same checks (silent install, upgrade, uninstall and the
  window click test), and makes a *draft* release that points at that commit; GitHub creates the tag `v<version>`
  when the owner publishes it. Publishing starts the workflow again through the new tag, which now sees the release
  is already published and does nothing. This was added because the tool that prepared the work was not allowed to
  push tags. Running it again refreshes the same draft: the files are replaced and the draft is re-pointed at the commit just built, so publishing it never puts the tag on an older commit than the files.

### Feedback round, 7 Nov 2026: twelve changes (made while the owner was away; the safest option each time)

The owner asked for the app to be as friendly as possible **and** to need the least maintenance. Every item below was
decided with those two rules.

- **Two scroll bars on General, and the "inside" bubble.** Fixed (the page scrolls in one place); the title bubble was
  cut off and said nothing, so it is gone.
- **Setup reads the default sound output.** On Windows the page asks for the current default output device by name and
  says whether videos can include its sound (Stereo Mix or a similar loopback device must be on). It is read when the
  page opens and by *Check again*; nothing is changed on the PC. Only unit-tested and run on Windows CI; not yet seen on a real PC.
- **Connections moved under Setup.** One page, "what CargoFrame uses, then where you share to". The old address
  `connections` still lands there and scrolls to it; the tab and the guide use the same page.
- **Discord without a bot (webhooks).** The easy way is a webhook link per channel (made in the channel's settings,
  pasted once). A bot token stays as the last resort for people who want the lists. Posting **through the person's own
  Discord account** was declined: automating a user account breaks Discord's terms and can get the account closed. A
  webhook cannot list servers, channels or threads, so: the channel is one of the links the person added, a forum is
  posted to by *thread name*, an existing thread by pasting its link. Slack has no webhook mode here because a Slack
  webhook cannot attach files; Slack stays a bot token.
- **Dependent dropdowns.** A dropdown can *narrow by* another (server, then category, then channel or forum). Each level
  can be *optional* and is skipped when empty; with no server the share is saved locally. Where a service gives no list,
  the person types, searches or pastes. Set in the layout editor, per dropdown.
- **Last choices are remembered.** The last five per dropdown, newest first, the list starting on the last one used.
  Kept on this computer in the window's local storage, per layout and block: values and labels only.
- **Forum and thread names as a search.** Titles can follow a convention (`[PROJ-123] short title`), and the typed text
  searches the known threads. The Discord guide proposes the convention in one sentence.
- **A button to get the token.** Beside each sign-in field a button opens the service's own page. All the addresses are
  in one file (`ui/src/lib/connectorHelp.ts`): the one place to change when a service moves a page.
- **A local program a connector needs.** The Claude desktop app is listed on Setup and on the connection's form; on Windows
  one button installs it with winget (the same fixed command shape as ffmpeg), and the download page is always one click away.
- **Screenshot and video in one layout.** The video block has *Also take a screenshot when the recording ends*; the
  screenshot block is empty until then; the video block has a *Take screenshot* button. A still goes to Jira, Slack,
  Discord, Google and the local zip. Teams, Reddit, Claude and Custom take pictures only: there the screenshot is sent
  when the video box is unticked, and the window says so.
- **In-app guides.** A guide for each connector and for the first capture, *Data from files* and *Destination fields*,
  opened from Settings only. It goes to the right page, boxes the field in the accent colour and says one short
  sentence. The steps are data (`tours.ts`), so a guide is edited by changing text; tests fail when a step points at a
  field that no longer exists, so the guides stay true without anyone remembering to check them.
- **Discord is two files.** The bot way stays in `discord/mod.rs`; the webhook way (about 300 lines) is in `discord/webhook.rs`, so
  a change to one cannot disturb the other.
- **Kept simple on purpose:** no new settings file, no new background process, no new dependency for any of the twelve.

### Second 7 Nov 2026 round: one path to post, what the computer plays, clean-up, formatting

The owner tested the installer ("works well") and sent twelve more points plus "there is no way to reset what the
install stores". The same two rules applied: as friendly as possible, as little to maintain as possible.

- **What the computer plays (bug).** ffmpeg on Windows can only record sound *inputs* (microphones, Stereo Mix), which
  is why videos had no sound and the Windows sound settings opened on the Recording tab. Fixed at the root:
  `core/video/src/loopback.rs` asks the Windows sound system for what the speakers play (WASAPI loopback, crate
  `wasapi`) and hands the samples to ffmpeg through a local TCP feed (`pcm.rs`). The video block has **Sound from**:
  everything the computer plays (default), or the program in focus (its process and the programs it started; if it
  cannot be found, everything is recorded). No device has to be switched on. The sound button opens the **Playback**
  tab. Linux still uses PulseAudio's monitor. *Only compiled on Windows CI and tested on Linux with a fake source; it
  needs one run on a real PC.*
- **One path to post (Discord and the rest).** A layout has **one** "where to post" dropdown per connection (validation
  refuses a second). It is a path: server, then category (optional), then channel or forum. Choosing a lower level no
  longer resets a higher one; a higher one resets only what is below it.
- **Threads are the second step of the path.** After a channel or forum, a **thread or post** box offers *New message or
  post* first, the threads *used before*, and the listed ones. It is searched (lists can be long), narrowed to titles
  that contain the Title text being typed (with *Show all* to undo it), and accepts a pasted link or id. The layout
  stores this as `deeper` on the dropdown, so a connector with something "inside" a choice can use it without new UI.
- **Why thread posts failed, and what changed.** A Discord webhook posts to *its own channel*; a thread belongs to that
  channel, so a thread sent through another channel's webhook is answered "unknown channel". Thread posts now go through
  the webhook of the thread's own channel (`thread_id` on the call), and a thread link is read for both ids. This could
  not be run against real Discord here; the unit tests use a mock that behaves as the documentation says.
- **Webhooks cannot list threads.** In webhook mode the list is what was posted to before (remembered) plus a pasted
  link. A bot token lists the threads. Said on the page instead of failing quietly.
- **The last path is the default.** The last choice at every level of the path (channel and thread included) is the
  starting point of the next capture, with the last five in the list.
- **Layouts saved before still work.** A layout with a separate Channel dropdown and Thread dropdown keeps working (the
  builder shows a warning on the thread box). A second dropdown for the same list is refused when saved. Nothing is
  rewritten behind the owner's back.
- **"Show me how" links are gone; Guides stay.** One way in, in the Settings menu.
- **Data from files is a sentence.** For a text file, *Between two pieces of text* shows **Text before it, The value,
  Text after it** (empty = to the end of the line; a filled one lets the value run over lines). The search pattern is
  built from the two texts (`settings/between.ts`); the regular-expression format stays for people who want a pattern.
- **"Destination fields" became "Extra fields".** The path to post is now chosen in the layout; what is left on that page
  is boxes beyond it (Jira Priority, labels). The page, its files and its guide are kept so nothing saved breaks; only
  the words changed.
- **Setup checks show a spinner, then a tick.** The page runs the checks every time it opens; each row shows a small
  spinner on the right, then a check or a warning. Details are folded away until the row is opened.
- **Notifications can be turned off.** General has two switches: *Say when something happens* (started, recording,
  shared or saved) and *Say when something goes wrong*. Both default to on. Settings: `General.notifications`.
- **Your data.** General, *Your data* lists what CargoFrame keeps on this computer with its size: work in progress,
  kept zips, logs, downloaded updates. Each kind has a clear button. *Reset CargoFrame* deletes settings, connections,
  sign-ins and data, and keeps the program and the folder the person chose for saved zips. It only touches the two
  folders CargoFrame owns (`core` path checks in `app/src/cleanup.rs`, tested with temporary folders).
- **Text formatting, per service.** A plain-text box in the window has buttons for bold, italic, underline, code, quote,
  bulleted and numbered lists (and Ctrl+B, I, U). The text is stored in one neutral markup; each connector converts it
  when it sends (`core/connectors/src/richtext.rs`: Slack `*bold*` and `_italic_`, Jira `+underline+` and `{{code}}`,
  Teams HTML, Discord and Reddit Markdown). Only the buttons that every service the box is sent to can show are offered
  (Slack and Reddit have no underline), and a box sent only to a title gets none. A layout can switch the buttons off per
  block. **Colour is not offered**: only Jira and Teams could show it, so most services would silently drop it.
- **Kept simple on purpose:** no new settings file, no new dependency apart from `wasapi` (Windows only), no new
  background process.

### Flow review, 8 Nov 2026: Settings by content, and what a large company needs

The owner asked for duplicated features to be found, the Settings sections to be organised by what they hold, a code
review, and the app to suit companies of thousands of people (connectors that can be left out of a build, other
programs handing data to a layout, company sign-in, accessibility).

- **The sidebar is in three groups, in setup order.** *Get started* (Setup, which still holds Connections, as asked on
  7 Nov), *Captures* (Layouts, Triggers, Data from files, Extra fields, Recording) and *CargoFrame* (General, Your
  data, About). Layouts now come before Triggers because a trigger picks a layout. A new window opens on Setup.
- **General was split by content.** General keeps appearance, startup, notifications, *When sharing fails* (the old
  Safety box) and *Windows kept ready* (the memory setting). *Your data* holds what is kept, clearing, reset, and
  export and import. *About* holds the version, updates and the speed and memory numbers. An update notice opens About.
- **The second video editor is gone.** Settings, Video had its own *Record until I stop* and *Trim, crop and mark a copy*,
  written to try ffmpeg before the CargoFrame window could do it. The window does all of it now, so the page (renamed
  *Recording*) keeps ffmpeg, capture and compression, and a 5-second test. About 1,600 lines and five backend commands
  went with it. When ffmpeg is missing the page sends you to Setup, the one place that installs it.
- **Linux trusts the system's certificates.** Windows already used the operating system's store; Linux used a list built
  into the program, so a company root certificate (network inspection) broke every connector there. Both now use the
  system store (`platform-verifier` in `core/net`).
- **The CargoFrame window's commands are an exact list.** The test was a list of forbidden commands that had gaps (newer
  commands such as the reset were not on it). It now compares the window's grant with the sixteen it needs, so a new
  command can only reach that window on purpose.
- **Planned, not built in this round:** fold Extra fields into the layout's routes; let the layout alone say what a
  capture records; one Cargo feature per connector; a managed folder and `policy.json`; `cargoframe open` for other
  programs. Each is described in `design.md`, *Flow review* and *For companies*.

### Connector round, 8 Nov 2026 (on a branch, for the owner's review)

- **One Cargo feature per service**, in `core/connectors` and forwarded by the app. The settings models stay in every
  build, so a saved connection of a service left out still reads and says "not in this copy of CargoFrame" instead of
  breaking the file. The window asks the program which services it has (`app_info.connectors`) and hides the others'
  forms, templates and guides. The forms' code stays in the window (a few KB); the code that talks to the service does
  not.
- **X**: the API v2 with OAuth 2.0 PKCE and the person's own Native App, because X gives no shared app and charges per
  post. Pictures only for now (`/2/media/upload`, then `/2/tweets`); a picture description through `/2/media/metadata`.
- **Bluesky**: handle and app password, because it needs no developer app at all, the easiest setup of any service.
  The text's links are marked (facets, counted in bytes) so they show as links.
- **Instagram**: Meta's API only takes a picture from a public address. Running a public server on people's
  computers was ruled out (security, firewalls). So the picture goes to a server the person or company owns (an HTTP
  PUT with an optional key header), then Instagram is told where it is, CargoFrame waits until Instagram has fetched
  it, and publishes. The token comes from the Meta dashboard (no OAuth window to keep working) and is renewed for 60
  days at each test or post.
- **Pictures that are too big are halved until they fit** (PNG, so text stays sharp); Instagram gets a JPEG, its only
  format. One small new dependency, `jpeg-encoder`, only in builds with Instagram.
- **Plain text** for X, Bluesky and Instagram: the formatting marks are taken out (a bullet becomes "•"), and the
  window offers no formatting buttons for them.
- **Other programs fill a layout** through `cargoframe open --trigger <id> --set <block>=<text>`: the single-instance
  channel already carries a second launch's arguments to the running copy, so there is no port, no service and
  nothing new to maintain. A trigger rather than a layout is named, so the calling program is captured as with a
  shortcut. Off for every layout until switched on.
- **Fixed on the way:** saving a template said "Saved as a copy" instead of "Saved as your own layout".

### Version 0.2.0, 8 Nov 2026: the layout decides, and a window that never drew

- **A shortcut records or takes a screenshot from its layout.** The owner found the trigger's *Makes* (screenshot or
  video) redundant with the layout and easy to set wrong. Now a layout with a Video block records (a press starts, the
  next one stops) and any other takes a screenshot. The trigger keeps *Screenshot of* (the program in focus, the screen
  under the mouse, all screens), shown only for a screenshot layout. Old trigger files keep their `media` field, which
  is read but no longer used, so nothing breaks and nothing is rewritten.
- **A window whose layout had a dropdown stayed on "Loading…" (fixed).** Since the thread step of 7 Nov, the window
  binds each dropdown's second step, and the value was missing until the person chose something; Svelte refuses to
  bind a missing value to a component that has a default, so the window never drew. Every dropdown now starts with an
  empty second step. The tests of single blocks could not see it, so the end-to-end check on a virtual screen now uses a
  layout with a dropdown (it failed without the fix and passes with it), and the whole window is drawn in a test for
  each kind of layout.
- **0.2.0** gathers the flow review, the connector round (X, Bluesky, Instagram, a build switch per service,
  `cargoframe open`) and these two changes.


### Wayland, 8 Nov 2026: through the desktop, not around it

Wayland lets no program listen to keys, read the screen or ask which window is in focus. Before this, CargoFrame took
a portal screenshot of the whole screen, its shortcuts never fired (the plugin only hears X11 programs), recording was
refused and pasting into Claude failed. Each is now asked of the desktop, and the program picks the way by itself.

- **Shortcuts are handed to the desktop**, which runs `cargoframe open --trigger <id>`; the running copy takes it over
  (the same path other programs use). GNOME: custom shortcuts in its keyboard settings, because every GNOME version has
  them and its GlobalShortcuts portal only arrived in GNOME 48. sway and Hyprland: bound live over their IPC socket at
  each start, so their config files are never written. KDE and others: the portal's GlobalShortcuts. Where none of
  these exists, Settings shows the command, with a Copy button. The plugin is not used on Wayland: it would only hear
  keys pressed in XWayland programs, which looks like a bug.
- **The window in focus comes from the compositor** (sway and Hyprland sockets, a short KWin script on KDE) or, on
  GNOME and others, from XWayland's active window, which the compositor clears when a Wayland window takes the focus.
  GNOME has no way at all for Wayland windows; the window says so instead of a vague "could not be read".
- **`grim` first on wlroots desktops**: no portal round trip, and sway set up with only the wlr portal has no
  screenshot service at all (it needs the GTK portal beside it).
- **Recording goes through the portal's screen sharing and GStreamer.** ffmpeg cannot read the PipeWire stream;
  GStreamer's `pipewiresrc` can. GStreamer writes raw video to CargoFrame, which passes it to ffmpeg over a local
  connection (as the Windows sound already is), so the recorder, the ring of segments and the export stay the same.
  The node is opened by number, not through the portal's file descriptor, because handing a descriptor to a child
  process needs unsafe code; a desktop that only allows the descriptor would need that changed.
- **GNOME and KDE allow no program to press keys in another**, so a Claude share there opens the chat and leaves the
  message on the clipboard, and says to press Ctrl+V. sway and Hyprland use `wtype`.
- **Tested on a headless sway desktop** in CI (`scripts/e2e/wayland-capture.sh`), also run as a GNOME-like desktop to
  cover the XWayland focus, the portal screenshot and GNOME's settings. Real GNOME, KDE and Hyprland sessions are the
  owner's to try.

### 0.3.0, 8 Nov 2026: data from files as a flow, links in the guides, formatting you can see

- **Data from files is a flow, not a format.** A rule now reads: *If this file is found* (its name, part of it or a
  pattern; the newest, or all of them) *then* add it to the files and/or read values. A value is one of three things
  people know how to describe: what is written after a name (`GPU: RTX 4070`, `"version": "1.2"`, `name = …`), the
  text between two pieces (the end is the end of the line unless one is given), or the whole line with a word; the
  first, the last or all of them, with a text for when nothing is found. There is no format to pick: the kind of file
  is known from its name, so a JSON, YAML, TOML, INI or XML file is read by key and anything else as text, and a
  broken JSON file still gives what is written after the name. Each value is its own variable, `{{rule.<key>}}`,
  named after what the person calls it. Files found by a rule that adds them are ticked in every Files block. Rules
  made before 0.3.0 keep working as they were (one value, keyed by the rule's id) and can be made simpler in one click.
  The machine's configuration (a `system_info.txt` with `GPU:`, `CPU:`, `RAM:` lines) is the example the page leads with.
- **Guides link to the web page they talk about.** A step that used to say "the button beside it opens the page" now
  shows the page as a link in its bubble ("Create a token"), opened in the browser. The forms' buttons and the guides
  share one list of pages, so a page that moves is changed once.
- **The formatted box shows its formatting.** A Text block gets *Message of* ticks in the layout builder, one per
  sharing button to Jira, Slack, Discord, Reddit or Teams: a ticked box is that button's message (its description,
  message or text), and only then gets formatting. Before, it needed a field mapping made by hand on the button, so
  the buttons seemed not to work. The box draws bold, italic, underline, code, quotes, bullets and numbers as they
  look; the `**` of a mark show only while the cursor is at it, so they can still be typed over. The text itself stays
  the marks, so the conversion per service (`richtext.rs`) is unchanged and the box reads the marks exactly as it does.
  Shortcuts: Ctrl+B, Ctrl+I, Ctrl+U, Ctrl+E (code), Ctrl+Shift+9 (quote), Ctrl+Shift+8 (bullets), Ctrl+Shift+7
  (numbers), as in Slack and Google Docs; Enter goes on with a list and ends it on an empty item; a paste comes in as
  plain text; Ctrl+Z and Ctrl+Y undo and redo. Only the marks every ticked service can show are offered (no underline
  for Slack or Reddit). A box that is nobody's message stays a plain text area.

### 0.3.0, 9 Nov 2026: Data & fields, formatting in the layout, GitHub

- **Extra fields become *Fill fields*, beside *Read from files*.** One sidebar entry, *Data & fields*, with two tabs,
  because the second mostly uses what the first finds. A field fill is one field of a service and the steps that
  fill it on every share: *Always* or *If* a value (one read from a file, the program, the project, or the field
  itself) *is found / is not found / is / is not / contains* a text, *add* or *set* a text, *and* more. *Add* is one
  more value in a list field (a label, given once), a new line in a description, or words after a title. It applies
  in every layout that shares there, or in the ones ticked. The same file (`infotypes/<id>.json`) keeps the field's
  kind, list and *required* for a custom field, so nothing old is lost: a starting value from before is shown, and
  saved, as the step "if this field is empty, set it to …".
- **No duplicate ways to fill a field.** A block of the window is sent in a field from the sharing button (the one
  list in *What this button does with each block*); everything that is not a block is a field fill. The button's
  old list (a field from a block, a value from a file or a fixed value) is gone: a value set there before 0.3.0 is
  listed under *Fields filled on each share* with *Move to Fill fields*, which makes the same fill for that layout.
  Until moved, it is still sent as before.
- **Formatting in the layout builder.** A Text block that is a message (*Message of*) gets, in the builder, the same
  formatted box and buttons as in the capture window, so a starting text such as ``Seen in `{{app.exe}}`
  **({{project.name}})** on *{{capture.date}}*`` is written formatted once; each capture fills in the values inside
  the marks. `{{variables}}` are shown as such in the box. The ready-made layouts for Jira, GitHub, Slack, Discord and
  Teams start with that formatted line. The value-from-a-file list there offers each value (`{{rule.gpu}}`), not
  each rule.
- **GitHub connector.** A fine-grained token (Issues, and Contents for pictures), a repository (another one can be
  picked per layout), GitHub Enterprise Server by its API address. A share opens an issue (title, Markdown
  description, labels, assignees, milestone, issue type) or, with an issue number, comments on it and adds the
  labels. GitHub's API cannot attach files, so the screenshot, a recording and the zip go in a branch of the
  repository kept for them (`cargoframe-attachments`, made from the main branch the first time) under
  `cargoframe/<day>/`, and the issue shows the picture and links the files; the connection can instead send text
  only. Each file and the issue are recorded as done, so a retry never doubles them. Underline is written `<ins>`;
  quotes and lists get the empty lines GitHub needs. Two templates: *GitHub issue* and *GitHub comment on an issue*,
  and a guide.


### 0.3.0, 10 Nov 2026: the polish round

- **Fixes.** The guide menu no longer adds a second scroll bar (the sidebar scrolls on its own, the page does not).
  The layout builder gives the grid and the block settings half the width each when there is room, one below the
  other when there is not. Text on a screenshot can be typed again (a click no longer took the focus away from the
  box it had just opened) and has bold, italic and *On a box*; clicking a text brings it back to change.
- **The guides are grouped.** The usage guides come first; the ones for connecting a service sit under *Connect a
  service*, below them.
- **A sign while recording.** A red outline around what is recorded and a red dot blinking once a second (each can be
  switched off in *Recording*, the dot's corner chosen). They are native windows, not web pages, so they cost almost
  nothing: on Windows left out of every capture, so never in the recording; on X11 drawn outside the recorded window;
  on Wayland not possible. This needs `unsafe` Win32 calls, kept in one file of its own crate (`core/indicator`).
- **The recording follows the focus.** While recording, which program is in front is noted about once a second; when
  it ends, the one used longest names the capture and gives the project folder, and the window says which programs
  were used. Nothing is read from a program that was only passed through.
- **Video crop and marks like the screenshot's.** Crop is a frame dragged on the picture, then *Confirm*; the single
  ring became marks drawn with the screenshot editor's tools (pen, square, circle, arrow, text, colour, size), laid
  over the export by ffmpeg.
- **Several subfolders in a Files block**, separated by commas, with a note under the box.
- **Text sent in a field.** A Text block says in which field of each sharing button it is sent (*Sent in*: a Jira
  summary or description, a Slack message…); the formatted box follows from the field that is a message.
- **The values list.** The starting text of a Text block is written with every value listed on its right (one above
  the other when the panel is narrow): a search box, the most used first (date, time, program, project, window
  title), then the program, the project, the capture, values from files, pictures in the text and links. The quick
  buttons and the two drop downs are gone, as they listed the same values twice.
- **Tables and code blocks.** Each service that can show them gets them its own way (a Jira table and `{code}`, an
  HTML table for Teams, Markdown for GitHub and Reddit); Slack and Discord get code blocks, and a table as aligned
  lines in one. In the box a table is a grid: type in a cell, Tab to the next, ＋ for a row or a column, × at the top
  of a column or the end of a row to take it out, as in Jira. The text keeps it as `| a | b |`, so nothing else
  changes.
- **Pictures in the text.** `{{capture.screenshot}}` and `{{capture.recording}}` put the picture or the recording where
  they are in the text, for the services whose API allows it: Jira (attached once, shown with `!name!`, even inside a
  table), GitHub (from the attachments branch) and Teams (the picture). The builder offers a *Repro | Highlight* table
  with both. Elsewhere they are left out of the text and the files are attached as before, so one layout works for
  every button.
- **Read text.** A tool on the screenshot: drag over text, the lines found are marked on the picture, copy them all or
  click the ones to copy, or *Try again*, which reads it another way (larger, more contrast, black and white, and
  for tesseract another page mode). Reading is done on the computer: Windows' own reader (no download, in the
  languages of the profile) or tesseract on Linux, which the person installs; nothing is sent. A crate of its own,
  `core/ocr`, so the capture window gets one command and no new permission.
- **Wider layouts.** A layout has 12 columns unless it says more, up to 24: *Columns ＋* beside the blocks adds one on
  the right side, *−* takes the last one away while no block is in it. A layout with more columns opens a wider
  window (no wider than the screen). A layout of 12 columns is saved as before, without the count.

### 0.3.1, 10 Nov 2026: a patch

- **Fixed: a Text block's settings could not be opened in the layout builder** once a data rule read a value from a
  file. The app lists those values among its variables and the values list added them again from the rules, so the
  same value was listed twice and the panel stopped drawing. Each value is now listed once (with the name its rule
  gives it), and a test covers the variables as the app sends them. The fields a sharing button offers are also
  listed once each.

### 0.4.0, 12 Nov 2026: tags, logs, files by hand, combined values, continuous recording

- **A Tags block**, like Jira's labels. At most 25 per block (and 100 characters a tag), kept once each whatever the
  case, so a field never gets the same label twice. It starts from a list in the layout where each entry may use a
  value: `{{rule.gpu}}` tags the graphics card when it is found and gives nothing otherwise, and a value with several
  parts gives several tags. Conditional tags ("if this, tag that") come from values made from other values (below),
  so the Tags block itself stays a plain list. The last five tags used are offered under the box, kept on this
  computer like the last dropdown choices. A block can suggest a list of a connection while typing (Jira's labels,
  a repository's labels); the Jira and GitHub templates start with one.
- **Where tags go, per service.** In a field that takes several values. Jira and GitHub have *Labels*; X, Bluesky and
  Instagram get a *Hashtags* field (added at the end, the text cut first so they stay; Bluesky marks them as tags);
  Slack, Teams and Discord get a *Tags* field, written as a last line. Discord forum tags need the forum's own ids and
  Reddit has one flair, so neither is set from tags yet.
- **A log viewer block.** It reads only a file the capture offers (found by a Files block or added by hand), from the
  capture's copy when there is one, and only the last 4 MB of a larger file. The search keeps the lines with all its
  words, case aside, and marks them; the last 2,000 matching lines are drawn. Nothing of it is sent: copying is the
  way out. It has no sharing boxes.
- **Files added by hand.** The ＋ of a Files block picks files with the system's dialog. Each is copied into the
  capture's own folder at once and listed under `added/`: everything that reads a ticked file finds that copy first,
  so a file chosen anywhere on the computer is never read from there again, and a share checks that a ticked file is
  one the capture offered, as before. 50 MB at most per file.
- **Values made from other values.** Data from files had one value per thing found. A value can now be made of others,
  with choices tried from the top: with no condition a choice fits when every value it names is found, so
  `{{rule.stream}}_{{rule.change}}_{{rule.shelf}}_{{rule.config}}` then `{{rule.stream}}_{{rule.change}}_{{rule.config}}`
  covers "with a shelf, else without" without a condition; conditions (is found, is not found, is, is not, contains)
  are there when needed. They are made after every file is read, with every value the capture knows, in order, so one
  can use another. A rule of only such values reads no file.
- **Continuous recording** (off by default). Holding a shortcut for two seconds asks to record all the time; holding
  one again stops. A press of a video shortcut then hands out the last seconds: the finished one-second pieces are
  copied while ffmpeg goes on (at most a second is not in it yet) and joined into a clip the window opens. Any other
  shortcut does what it always does, and each opens its own layout, whatever started the recording. With the setting
  on, shortcuts act when let go; with it off nothing changes and a press acts at once. Wayland does not say when a key
  is let go, so the tray menu starts and stops it there (and everywhere).
- **The focus in a continuous recording.** A window's place and size can change while it runs, and pieces of
  different sizes cannot be joined without encoding again, so the whole screen is recorded. The red outline follows
  the window in focus, and a layout that records the window in focus opens cropped to the one in focus the longest in
  the clip, a crop the person can change or take away. The program in focus the longest gives the data, as for any
  recording.
- **The sharing boxes and the ring.** The boxes of the sharing buttons start at the block's top-right corner, their top
  on the block's top; the ring of the hovered button goes around what the block holds, not its title.
- **The keys pressed, over a recording** (a switch on the Video block, off by default). While it records, a thread
  looks at the keyboard fifty times a second, the way the system answers "is this key down" (`GetAsyncKeyState` on
  Windows, `QueryKeymap` on X11), not with a hook into other programs' input; presses stay in memory with their time,
  as long as the recording keeps pictures. Keys pressed close together make one line ("hello ⏎", "Ctrl+S"), shown from
  each press until 1.5 s after the last. The window plays them over the clip where the person puts them (or takes them
  off); for the share it draws each line as a picture, all the same size, black with a white outline, and ffmpeg lays
  them over the sent picture at their times from one list (`concat`), after the crop, so they stay where they were
  put. Wayland does not let a program see other programs' keys: the recording says so and goes on. Every key shows,
  passwords too, and the switch says it.
- **A window that is moved** (*Settings, Recording*, off by default). A recording of a region cannot move, so with
  this on the whole screen is recorded and where the window is gets noted ten times a second (`WindowPlace`, one
  connection kept, the window asked by its own id, so another window taking the focus changes nothing). The clip opens
  cropped to it and the crop slides from place to place as it plays and in the sent clip (a sum of ramps ffmpeg works
  out for each frame). It costs a larger picture to encode, hence the switch. A crop set by hand stops following;
  *Follow window* brings it back. The same places serve a continuous recording that has the setting on. Recordings
  made smaller (the height setting) now crop at that scale too: the crop to the window in focus was off on screens
  taller than the setting.
- **Text blocks keep one line.** In a short window a block's share of the height could leave its box a few pixels high;
  the box now keeps at least one line (the block scrolls instead), and *One line high* keeps it at exactly that.
- **Searching the settings.** A box above Setup (Ctrl+F). While it has words, the sidebar lists only the pages and
  settings that have them, and the sections of the page shown that do not are greyed out rather than hidden, so
  nothing moves under the mouse; a result opens its page and lights the setting up. What can be found is a list
  (`settings/search.ts`) checked against the pages' own section titles and `data-tour` names, so it cannot point at
  something that was renamed; the page shown is also matched by what it says on screen.
- **Tidying, kept to safe steps.** Slack, Teams and Discord share one function for the words of a message; the tests of
  `share.rs` and `session.rs` moved to files of their own (3,000 lines); the trigger code reads the recording settings
  in one place; an unused style went.

### 0.4.1, 12 Nov 2026: the time of a recording, and the log lines written then

- **The time over a recording.** The capture now notes when it was made, and a recording when its clip starts on the
  computer's clock (the moment it was handed out, less its length and the part a continuous recording had not
  finished). The window shows the time of each moment over the clip (2026-11-12 14:03:27, the way logs write it), shown
  or hidden with *Time* under the recording, moved and taken off like the keys; the Video block can start with it
  shown. The share sends it the same way as the keys: one line per second, drawn as pictures by the window, laid over
  the sent picture by ffmpeg. Both now go through one command (`set_clip_layer`, with each line's time), so a text over a
  clip is one thing however many there are.
- **Log lines by time.** The log viewer reads the time at the start of each line (ISO dates with or without a zone,
  `2026/11/12 14:03:27,123`, Unreal Engine's `[2026.11.12-14.03.27:123]`, syslog's `Nov 12 14:03:27`, or the time
  alone on the capture's day); a line without one (the rest of a message, a stack trace) has the time of the line
  before. One row holds both searches: one button at its start turns it from the words (shown first) to the time,
  its icon being the search it turns to (a clock, then a magnifier); both keep applying, and a dot on the button says
  when the one not shown is on. The time search keeps the lines written *during the recording* (5 s either
  side, as the program's clock and the recording may not agree to the second), *around the capture* for a screenshot
  (the minute before), or *between* two times typed (a block can start on the recording; the row still opens on the
  words). Times without a zone are the computer's own, or UTC when the block or the person says (Unreal Engine writes
  UTC by default). A file whose lines carry no time it can read says so and shows every line; nothing else is guessed
  (a day written 12/11/2026 could be either way round, so it is not read).

