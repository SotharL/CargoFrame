# Security review (V1)

_Reviewed 2026-10-23, on the code merged to `main` in pull request #1. This is a code review plus tests, not an outside audit._

## What CargoFrame must protect

| Asset | Where it is | The rule |
|-------|-------------|----------|
| Sign-in secrets (API tokens, OAuth tokens, passwords) | The operating system's credential store (Windows Credential Manager, Secret Service on Linux) | Never in a config file, a log, a zip, the window, or an error message. The windows can say *whether* one is stored, never read it back. |
| The capture (screenshot, files, values read from files) | Memory, and a saved zip only when a share fails | Leaves the computer only when you press a sharing button. No telemetry, no background upload. |
| Files outside the project folder | The user's disk | Data rules and the files block read only inside the resolved project folder, read-only, through an allow-list. |

## How the pieces are separated

- **The tray process has no web view.** The Settings and CargoFrame windows are web views created on demand.
- **Web views get no plugin permissions.** `app/capabilities/*.json` grant each window only the app's own commands (and event listening). There is no file-system, shell, HTTP or opener access from JavaScript; the CSP also forbids connecting anywhere but the app backend.
- **The CargoFrame window is the less trusted of the two.** It cannot save items, set secrets, connect or disconnect, or change settings. It can only ask for what the open capture allows: the share uses the layout, connection, mappings and files **held by the backend for that capture**, not what the window sends.
- **Every network call goes through one client** (`core/net`), which enforces the rules in the next section.

## Findings

| # | Finding | Severity | Status |
|---|---------|----------|--------|
| 1 | The HTTP client let its library follow redirects without checking each hop. A service (or anyone able to alter its answer) could redirect an https request with a body (307/308) to plain `http://`, or to a local port on this computer, and the screenshot would follow. | Medium | **Fixed.** Redirects are followed by `core/net` itself. Each hop must pass the URL check (https; plain http only between local test servers; no user name in the address), may not leave https for http, and may not carry a sign-in header (`Authorization`, `Proxy-Authorization`, `Cookie`) to another origin. A 301/302/303 on a POST becomes a GET without the body. At most 5 hops. Tests: `core/net/src/url.rs` and `core/net/tests/client.rs`. |
| 2 | The content security policy did not forbid plugins, `<base>` tags, frames or form posts. Nothing in the app used them, so this was hardening. | Low | **Fixed.** Added `object-src 'none'; base-uri 'none'; frame-src 'none'; form-action 'none'`. A test (`shipped_config_tests` in `app/src/lib.rs`) fails if the policy is loosened, if a window is given a non-app permission, or if the CargoFrame window is given a settings or secrets command. |
| 3 | Nothing proved that a stored secret stays out of every file CargoFrame writes. | Low | **Fixed.** `a_stored_secret_is_in_the_secret_store_and_in_no_file` stores a token and searches every file under the settings and data folders. |
| 4 | A secret input sat inside another form (nested forms are invalid HTML). | Low | **Fixed.** It is no longer a form; Enter in the box stores the token and never submits the connection form. |
| 5 | The weekly dependency check (`cargo audit`) found three advisories in the `zip` crate, version 6 (RUSTSEC-2026-0367, -0368, -0369): extracting an archive could follow links outside the target folder or keep set-user-id bits. | Low for CargoFrame | **Fixed.** CargoFrame only *writes* zips and never extracts one, so it was not exposed, but `zip` is now version 9, which has the fixes. |

## Checked and found sound

- **OAuth (Google, Reddit):** one-time listener bound to `127.0.0.1` only, random `state` that must match, PKCE (S256) where the service supports it, sign-in can be cancelled, a wrong state is refused (`core/connectors/src/oauth.rs`).
- **Injection through templates:** values placed into a Custom API's JSON body are JSON-escaped, into paths and queries URL-encoded, and a request that mentions a field nothing fills is refused instead of sent with a hole. A header value with a line break is refused, not sent. Multipart field names cannot break out of their header.
- **Zip contents:** names with `..`, absolute paths or other unsafe forms are errors, not skips (`core/bundle`). A fallback zip holds the screenshot, the share description, the error, the ticked files and a manifest, and no secrets.
- **Reading files:** only inside the project folder, only paths the allow-list accepts, never writing.
- **Opening links:** only `https://`, `http://127.0.0.1`, `http://localhost` and `claude://claude.ai/` can be opened from a window.
- **Logs:** record events (trigger id, connection id, attempt count, failure class, timings). They do not record window titles, file names, payloads, headers or tokens. The log file is size-capped.
- **Errors from services:** HTML pages and very long messages are cut before they reach a window.
- **Slack:** the bot token is kept in the credential store and sent only to Slack's API address. The addresses Slack hands out for file uploads are used only when they are on slack.com (the token is not sent to them either), and a message's text is escaped (`&`, `<`, `>`), so a window title or a data rule cannot write `<!channel>` and notify a whole workspace.
- **Discord:** the bot token is kept in the credential store and sent only to Discord's API address. Every message sets `allowed_mentions` to none, so captured text cannot ping `@everyone` or a person. Messages carry a nonce so a retry cannot duplicate them; a forum post whose outcome is unknown is never retried.
- **Teams:** the sign-in is OAuth with PKCE and no secret (a public client). The refresh token is kept in the credential store and the access token is sent to Microsoft Graph's address only; a "next page" link that points anywhere else is not followed. A message's text is escaped, so a window title or a data rule cannot write markup or an `<at>` mention. A post whose outcome is unknown is never retried. Permissions are delegated and narrow (`ChatMessage.Send`, `ChannelMessage.Send`, and read access to the names of chats, teams and channels): CargoFrame cannot read the content of any chat.
- **GitHub (connector):** a fine-grained personal access token in the credential store, sent only to the API address of the connection (https only). The person limits it to the repositories it may use: Issues read and write, and Contents read and write only when pictures go in the attachments branch. Pictures and files are put in that one branch (made from the main branch, never the main branch itself), under `cargoframe/<day>/`, with names reduced to letters, digits, dots and dashes; whoever can read the repository can read them, so a public repository makes them public. A share that is retried never opens a second issue or puts a file twice.
- **Updates:** "Check for updates" (Settings, About) asks the GitHub releases API for the newest release and installs it only when the person presses the button. The installer is downloaded to CargoFrame's own data folder and its SHA-256 is compared with `SHA256SUMS.txt` from the same release before anything runs; a missing or different checksum means no install, with no fallback. Asset names and addresses are validated (an installer named like a path, or an asset address that is not under this repository on the GitHub API, is refused, because the token is sent to it). The GitHub token that a private repository needs is kept in the system credential store, never sent back to a window, and sent only to `api.github.com`; the download's redirect to the file host drops it. The checksum file travels with the installer, so it guards against a damaged or truncated download, not against someone who can replace both release files: that needs a signed installer (see `docs/signing.md`).
- **Video (Settings, Recording, and the CargoFrame window for a video trigger):** ffmpeg is a separate program the person installs; CargoFrame runs it as a child process with an argument list (never through a shell), and every file it is given is written `file:<path>`, so a name cannot become a protocol or an option. The page passes no path to any video command: the test clip lives in one fixed folder, `video-test/` in CargoFrame's data folder. A recording keeps only the last seconds on disk as small files, deletes the rest as it goes, and is stopped (ffmpeg killed, pieces removed) when Settings closes or CargoFrame quits; ffmpeg also stops itself after an hour if CargoFrame is killed. The test clip shows the screen, so it is a screenshot in motion: it stays on the computer, is replaced by the next test, and is never sent anywhere. The content security policy gains `media-src 'self' blob:` so the page can play a clip it received as bytes; the CargoFrame window holds none of the commands that start, test or export a recording (pinned by a test). It holds two that deal only with its own capture: `get_session_clip` (that capture's recording as bytes, at most 100 MB) and `set_clip_mark` (a PNG, checked, for the capture named in the request; a late call cannot reach a newer capture).
- **Video triggers:** pressing a video trigger's shortcut starts recording the screen (nothing was recorded before that press), and the next press stops it. A notice appears when it starts, the tray's tooltip says it, and the tray has an entry that throws the recording away. Only the last seconds set in Settings, Recording (30 at most) are kept; the rest is deleted as it goes. A recording started by a trigger has no ten-minute limit, because the person stops it. When it ends, the clip is moved into the capture's own temporary folder (deleted when the window closes) and goes only where the button sends it: the capability table keeps it from Reddit, Teams, Claude and Custom API, and a recording over 100 MB is refused before it is read. Cut, crop and mark are made by ffmpeg from numbers (times, a rectangle) and the PNG the window drew; the page never passes a path, and the cut copy and the mark are written in the capture's folder.
- **Recording sign and focus (0.3.0):** while a recording runs, a red outline and a blinking dot are drawn by the
  system's own windows (no web page). They take no clicks and no focus; on Windows they are excluded from every
  screen capture (`WDA_EXCLUDEFROMCAPTURE`), so they are not in the recording. This is the one place with `unsafe`
  code (`core/indicator/src/win.rs`, Win32 calls); that crate denies it everywhere else and every other crate forbids
  it. Following the focus during a recording asks only which program is in front, about once a second, and keeps the
  program names and times in memory for that capture; the program used longest picks the project folder, which is
  then read as for a screenshot.
- **Files added by hand (0.4.0):** a file is picked with the system's dialog (the window cannot name a path), copied
  into the capture's own folder at once (regular files, 50 MB at most) and offered under `added/<name>`. Every read of
  a ticked file finds that copy first, and a share still refuses a file the capture did not offer. The copy goes with
  the capture's folder when the window closes.
- **Log viewer (0.4.0):** the window asks for a file by the path it was offered (`read_offered_file`); anything else is
  refused, a path that leads out of the folder too (the same check as for a zip), and only the last 4 MB are read.
- **Keys pressed (0.4.0):** read only while a recording runs whose layout's Video block asks for them (off by default,
  and the switch says that passwords typed then show too). The keyboard is asked which keys are down; no hook is put
  into other programs' input. Presses stay in memory with their time for as long as the recording keeps pictures, and
  are dropped with it; only the lines the person sends are drawn into the clip. Wayland does not allow it.
- **A window followed (0.4.0):** where the recorded window is gets asked ten times a second while it records (its
  place and size only), kept in memory with the capture.
- **Continuous recording (0.4.0):** off unless turned on; starting asks first (a system dialog) or comes from the tray
  menu, and the tray and the outline say it runs. ffmpeg is told to stop after 12 hours. Only the last seconds are on
  disk, in the recording's own folder, as for any recording; a clip handed out is a copy that moves to the capture's
  folder.
- **Read text (0.3.0):** the part of the screenshot the person drags over is read **on this computer**: Windows' own
  reader (`Windows.Media.Ocr`) or, on Linux, the `tesseract` program run with an argument list (never a shell), the
  picture handed over on its input (no file), stopped after 20 seconds. Nothing is sent anywhere and nothing is kept:
  the text is shown to be copied. The capture window can call this one command (`read_text`), with a PNG of at most
  40 MB.
- **Pictures in the text (0.3.0):** `{{capture.screenshot}}` and `{{capture.recording}}` are placeholders the window
  keeps as they are; the backend replaces them with the attachment of that same share (a Jira `!name!`, a GitHub link
  to the attachments branch, a Teams inline picture) or removes them for other services. They never name a file on
  disk.
- **Dependencies:** `npm audit` and `cargo audit` report no vulnerabilities. A weekly and on-change workflow (`.github/workflows/security.yml`) runs both; its first run is what found finding 5. `cargo audit` still lists nine *warnings*, all "unmaintained" or "unsound" notices on libraries pulled in by Tauri 2 (GTK 3 bindings, two build-time helpers) and by the credential-store crate (D-Bus bindings, `zbus` and `zvariant`). They are not vulnerabilities and the workflow does not fail on them, but they are the list to re-read at each release.

## Known limits (honest list)

- **Credential store on Linux** needs a running Secret Service (GNOME Keyring, KWallet). Without one, a connection cannot keep its token and the Connections page says so. It has been exercised in unit tests only, not on a real desktop.
- **Signing.** The release workflow signs the Windows installer when the certificate secrets exist. The program inside the installer is **not** signed yet, and the beta certificate is self-signed, so Windows will warn. Untested until you add the secrets (see `docs/signing.md`).
- **Custom API connections are user-defined.** CargoFrame requires https and checks every redirect, but it cannot know whether the service you describe deserves the screenshot. The share button's destination is always shown by name.
- **An exported settings file** holds layouts, data rules and field fills only. It names the connection a layout uses, and may name files a data rule reads (`package.json`, `logs/*.log`) and text you wrote in a layout, so read it before handing it on. Connections, tokens, keys, triggers and shortcuts are never in it. Importing reads a file you pick (at most 4 MB, 500 items per group), checks every item like one typed into Settings, and changes nothing until you have seen the preview and chosen. The export, import and template commands are given to the Settings window only; the CargoFrame window cannot call them (checked by `shipped_config_tests`).
- **A saved fallback zip contains your screenshot and files** and lives in the safe folder you chose (or CargoFrame's own data folder). Protect that folder like the originals.
- **A leftover recording after a crash.** If CargoFrame is killed (not closed) while a test recording runs, ffmpeg can keep capturing for up to an hour (its own time limit) and leave small files in `video-test/ring`. Closing the program normally, or closing Settings, stops it at once. A job-object guard that ends ffmpeg with CargoFrame would need unsafe code, which the workspace forbids.
- **Wayland.** The screen is read only through the desktop: the portal (which may ask permission) or `grim` on sway and Hyprland; video through the portal's screen sharing, whose "remember this screen" token is kept in CargoFrame's data folder. To learn the window in focus, CargoFrame asks sway or Hyprland over their own socket, or loads a one-line KWin script on KDE that answers over D-Bus and is unloaded at once; it reads nothing else. Shortcuts are handed to the desktop: on GNOME CargoFrame writes custom shortcuts into the person's keyboard settings, only under paths that start with `cargoframe-`, and leaves every other one alone (the uninstaller removes them); on sway and Hyprland it binds keys live and never writes their config files. Each command it adds is `<program> open --trigger <id>`, with both words quoted for the shell.
- **Going public (9 Nov 2026):** the history was scanned for secrets (gitleaks, none found) and personal paths, and `SECURITY.md` says how to report a problem privately. Still to do: a licence, and a trusted certificate (`docs/signing.md`).

## Connector round (8 Nov 2026, under review)

- **`cargoframe open`** (other programs fill a layout): it travels through the single-instance channel of the
  running copy, so only programs of the same user on the same computer can reach it, and no port is opened. A layout
  must be switched to *Other programs can fill this*; otherwise the values are left out and the window says so. Values
  go into text boxes and dropdowns only, at most 50 of 20,000 characters, and are never sent by themselves: the person
  sees them in the window and presses the button. The log says how many values came, never what they were.
- **X**: OAuth with PKCE and no client secret (a public client), the person's own app; the refresh token (which X
  renews at each use) is kept in the credential store. **Bluesky**: an app password (never the account password) in
  the credential store, a new session for each share. **Instagram**: the access token and the picture server's key in
  the credential store; the picture is uploaded with a fresh random name to the address the person set (https only),
  and that address must be public, because Instagram fetches the picture from it. Anyone who knows the name can open
  that file: it is the post's picture, about to be public anyway.
- **Builds without some services** contain none of their code, so a service a company does not allow cannot be
  reached from that build.

