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

