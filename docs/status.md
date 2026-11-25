# Status

_Last updated 2026-11-25, version 0.7.2 (an update no longer undone at sign-in by an older copy in the machine's startup list; debug mode, for reporting a problem in CargoFrame itself; a value of several parts and grips on a selection; the Tags list over the blocks below it; the Columns control, the first tab's corner, the Fill fields form and the New value panel fixed; Jira people and teams searched by name, a new ticket assigned to you; the formatted text box fixed for Enter and a paste in the window; a recording of several screens kept to about the main screen's size; Tags picked from the service's list; copy, paste, duplicate and delete in the layout builder, and locked tabs; the computer's details as values; deleting several items in Files & backup; 0.7.1: updates found again on Windows; a followed window recorded on its own screen without redrawing its outline as it moves; shortcuts of any keys, watched by CargoFrame when the system cannot; ffmpeg downloaded and updated from System; templates that record the window in focus beside its screenshot, with starting texts that use the values and a recording/screenshot table; Jira and GitHub issues created or updated, each field as its mapping says, with a comment and a vote; blocks that must be filled before sharing; file tabs in Data parsing that close and stay until Settings closes; 0.7.0: Settings regrouped: Connections; Layouts, Shortcuts, Data parsing, Recording; Preferences, Files & backup, System, each page with an icon; a tour that starts after an install or an update; recording made lighter (the graphics card's capture and a hardware encoder where they work, ffmpeg below normal priority, one conversion); the log viewer reads a time selected in a line and follows the clip as it plays; a file dropped in Data parsing opens, and a long line no longer pushes the panel out of sight; the wording of the four languages brought to the terms technical tools use; 0.6.0: French, Spanish, Italian and German besides English: the computer's language by default and a setting in General, the packs checked on every push; French, Spanish, Italian and German besides English: the computer's language by default and a setting in General, the packs checked on every push; 0.5.1: the ready-made layouts brought up to date: one-line titles, `{app.title}`, hashtags for X, Bluesky and Instagram, the time over a recording and its logs from the recording; 0.5.0: values made by selecting them in a file of a capture, read in several files with the newest winning, written `{map}`; Data & fields as Values and Files, Fill fields a tab of Layouts, `{` and a values preview in the layout builder; 0.4.1: the time shown over a recording, the log lines written during it, read with time formats people write or data rules set; a Tabs block; the layout round: rows that hold one line, sharing dots, the recording's tools above it as icons, the log viewer opened large in the middle; 0.4.0: a Tags block and where tags go per service, a log viewer block, files added by hand, values made from other values, continuous recording held to start and stop, the keys pressed shown over a recording, a recording that follows a moved window, a search in Settings, Text blocks that keep one line, the sharing boxes in the block's corner; before that, 0.3.1, a fix: a Text block's settings did not open in the layout builder once a data rule read a value; before that, 0.3.0, the 10 November round: a sign while recording and the focus followed, tables and code blocks in the formatted box, the screenshot and the recording placed in the text for Jira, GitHub and Teams, the values list beside the starting text, *Read text* on the screenshot, wider layouts with up to 24 columns, the video crop and marks reworked, text on screenshots with bold, italic and a box, several subfolders in a Files block, the layout builder split in half, the guides grouped; before that, 2026-11-09: Data & fields: read values from files, then fill the service's fields with them by steps; formatting in the layout builder's starting text; a GitHub connector; Data from files as a flow: if a file is found, add it and read values, no format to pick; links to web pages in the guides; a formatted box that shows its formatting, with *Message of* on the Text block; before that, a shortcut records or takes a screenshot from its layout; fixed: a window whose layout had a dropdown stayed on Loading; X, Bluesky and Instagram against mock servers, one build switch per service, `cargoframe open`; the flow review: Settings grouped by content, General split into General, Your data and About, the duplicate video editor removed from Settings, Linux trusts the system's certificates; before that, 2026-11-07, evening: the second 7 November round after the installer test: one path to post with threads, sound recorded from Windows itself, Your data and reset, notifications switch, text formatting, Setup checks, Data from files as a sentence; before that, the twelve changes of the first 7 November feedback round: Discord webhooks, dependent dropdowns, last choices, get-token buttons, Claude app install, screenshot with video, in-app guides, Connections under Setup; before that, phases 8 to 12, including video, were merged to `main` in pull request #19; the rest dates from 2026-10-23). V1 was merged to `main` from `dev/v1` in pull request #1. The settings update (General and Safety, Save locally, layout templates, text and log rules, export and import) is in pull request #12._

**V1 is feature-complete in code. It has not yet run on a real Windows 11 or Linux desktop, and
no real Jira, Google, Reddit or Claude account has been used.** Everything below says which of
those is true for each piece.

## What is verified

- **Unit and integration tests:** about 1,110 Rust tests and 650 UI tests pass. `cargo clippy -D warnings`,
  `cargo fmt --check` and `svelte-check` are clean.
- **0.7.2 (25 Nov):** the sign-in entry pointed at the installed copy and a machine entry for another copy
  removed, or named in Preferences (`autostart.rs`, `Preferences.test.ts`); a value of several parts, the same
  in Rust and the window, on ffmpeg's lines (`find.rs`, `pick.test.ts`); the grips (`FileViewer.test.ts`) and a
  selection's stop (`pick.test.ts`); the floating list (`floating.test.ts`); debug mode: what a window reports
  (`debugView.test.ts`), the values and the log kept (`debug.rs`), the bug report template (`templates.rs`) and
  the switch (`System.test.ts`); in a browser: the Tags list, the first tab's corner, the Columns row, the Fill
  fields form, a narrow New value panel, a two-part value and the grips. **Not yet tried on a real PC:** the
  sign-in entry repaired on Windows (compiled by Windows CI) and the *Stop the older copy* button.
- **0.7.2:** Jira's people searched for what is typed (anyone, or in a project; apps and people gone left out;
  *Me* first, turned into the person signed in), teams through the Team field's search, and *Me* in a new
  ticket's assignee, against a mock server (`jira/tests.rs`); the window asking again as one types and keeping
  the choice (`DropdownBlock.test.ts`); the formatted box inside a layout cell taking Enter and a paste as text
  (`TextBlock.test.ts`), and in a browser: Enter, Shift+Enter, lists, a pasted text and a pasted web page, Escape,
  a table, undo; the recording size (the main screen's pixels, 3840 × 2160 at most) with real ffmpeg on 5760 ×
  1080, 11520 × 2160, stacked and small pictures (`recorder_tests.rs`), and 10-second recordings of a busy
  5760 × 1080 picture on one core and on two (`performance.md`); a followed window placed in the smaller
  picture (`watch.rs`); Tags picked from a list, several, by the mouse and the keys (`TagsBlock.test.ts`); copy,
  paste, duplicate, delete and locked tabs (`model.test.ts`, `Canvas.test.ts`); the computer read from Linux's
  files (`system.rs`) and listed in Data parsing (`DataFields.test.ts`); deleting several items with what goes
  and the shortcuts left named (`TransferSection.test.ts`). **Not yet tried on a real PC:** a recording of three
  screens on Windows (the crash could not be reproduced here: the size limits remove what made it likely), the
  Windows registry reading of the system and drivers (compiled by Windows CI), and the people and team search on
  a real Jira (the Team field's search on Jira Cloud and Data Center).
- **0.7.1:** the update check finding the installer under either name, and leaving out a token that no longer
  works for a public repository (`release/tests.rs`); a followed window recorded on its screen and cut to it
  (`watch.rs`); key combinations, the keys a shortcut can hold, a key alone, the modifiers left out, and the watcher's
  edges against a fake keyboard (`combo.rs`, `shortcut.rs`, `shortcut.test.ts`); the ffmpeg offer read from a
  GitHub release and the version compare (`ffmpeg_get.rs`; the download is the update's own checked download); Jira
  and GitHub updates (fields replaced, added below, one line, labels added, counted, left; comment; vote and 👍,
  one refused) and the old behaviour of a 0.7.0 layout against their mock servers; templates (every recording
  template records the window in focus beside its screenshot, the report table, one create-or-update template with a
  screenshot and one with a recording for Jira and GitHub, *Send to Jira* / *Send to GitHub*, what must be filled) in
  `templates.rs`; the last column, row and header row of a table removed (`TextBlock.test.ts`, `richtext.test.ts`); the required blocks in the share (`share_tests.rs`) and in the window (`willSend.test.ts`);
  the update choices and the checklist in the builder (`SharingPanel.test.ts`, `updates.test.ts`); the file tabs kept
  and closed (`files.test.ts`). The System ffmpeg line, the sharing panel and the Data parsing tabs (closed, then
  found again after leaving the value) were looked at in a browser with demo data. **Not yet tried on a real PC:**
  the key watcher on Windows (compiled by Windows CI) and on a real X11 desktop, the ffmpeg download and unpacking
  with GitHub's real files, an update of a real Jira or GitHub issue, and the 0.7.0 → 0.7.1 update itself.
- **0.7.0:** the format worked out from a selected time (`logInfer.test.ts`: the usual formats, day and month order,
  zones, letters that mean something, what is not a time), the log viewer reading a selected time and following a
  playhead (`LogViewBlock.test.ts`), the Settings pages, groups, icons, old addresses and the tour started from the
  address and skipped (`App.test.ts`, `route.test.ts`, `walk.test.ts` walks the tour like every guide), the tour due
  once per version (`setup_tests.rs`). Recording: the choice of capture and encoder, what the graphics-card capture
  can see, the reading of what ffmpeg refused (`video_tests.rs`), each encoder's arguments and the GStreamer pipeline
  (made smaller, checked with `gst-launch-1.0` on a test source). On Linux, the cost of capture and conversion and of
  x264's presets was measured with ffmpeg on a virtual 2560×1440 screen (about 0.4 of a core each at 30 frames a
  second for the conversion and for ultrafast; veryfast costs nine times ultrafast). **Not yet tried on a real PC:**
  Desktop Duplication instead of GDI, the hardware encoders and their test picture, the lower priority, and a file
  dropped on the Settings window (the system's own drop handler is turned off for it), on Windows and on a Linux
  desktop.
- **0.6.0, languages:** `scripts/i18n/check.mjs` finds every text of both codebases in all four packs with the
  same `{names}` (it runs in the local gate and CI). Rust tests switch to French and German and back; UI tests load
  the French and Spanish packs and draw General in Spanish. Settings (General, Layouts, Triggers, Recording, Setup),
  the layout builder with a template and the CargoFrame window were looked at in each language with demo data, and a
  scan of every page in French found no text left in English that has a translation (the demo's own sample data
  aside). The translations were written by Claude, file by file, with what each control does in view; no native
  speaker has read them yet.
- **The 7 November round** (Discord webhooks and bot, dependent dropdowns, last five choices, get-token buttons, the
  Claude app line, screenshot with video, the guides): unit tests on each piece, connector tests against the mock
  servers, and the guides are walked step by step in a browser test (`walk.test.ts`) and checked against the
  fields that exist (`tours.test.ts`). The capture window with a recording and a screenshot, the Settings Setup page
  with the Discord form, and a Jira guide were also looked at in a browser with demo data.
- **The second 7 November round:** the path with a thread step (the layout rules, the Discord webhook routing of a thread
  post to its own channel, the remembered posts, the pasted link) against the mock server; the text converter for each
  service (`richtext_tests.rs` and each connector's tests); the clean-up on temporary folders; the toolbar, the Setup
  rows and the data page against the in-memory stand-in; the Windows sound capture is compiled by Windows CI and its
  feed is tested on Linux with a fake source (`pcm_tests.rs`). A new test checks that every command the web view can
  call is registered, listed and given to a window (it found the data buttons were not granted before this was fixed).
- **0.4.0:** tags are cleaned and capped in one place (`core/layout`), tested with the starting list and in a share
  to a mock service; hashtags, Bluesky tag facets and the Slack tag line have tests of their own. Combined values are
  tested in the rules crate and through the rule preview with real files. Files added by hand go through a real share
  saved as a zip (the copy is what goes, the file changed afterwards is not), and the log viewer only reads what the
  capture offers. A continuous recording on a virtual X11 screen hands out two clips while it goes on; hold
  detection, the crop to the window in focus and the tray text are unit tested. Holding a real key, the dialog and
  the outline following the focus have not been tried on a real desktop. The keys are read from a real X display
  with `xdotool` (an ignored test: `xvfb-run cargo test -p cargoframe-keys -- --ignored`); their lines, the pictures'
  framing and a real ffmpeg export that shows them at their time and follows a moved window (whole and cut) are
  tested, as is a share that sends them. The Windows keyboard and window-place code is compiled by Windows CI only.
  The settings search is checked against the pages' code; a Text block's height was measured in a browser (10 px
  before, one line after). The time over a recording (0.4.1): its lines per second, the shared upload of texts over a
  clip and a share that sends the keys and the time are tested; the log times are read from each format in tests,
  and the time row was looked at in a browser. The layout round (0.4.1): the Tabs block's checks are tested in Rust
  and its moves in the builder's model; the window test opens a layout with tabs and switches them; row heights have
  tests; one-line titled texts, long button labels, the large log view, the sharing dots, the ring round the picture
  and the tabs on each side were looked at in a browser with demo data, not yet on a real desktop.
- **0.5.0:** a value picked in a file is found the same way by the capture (`core/rules/src/find.rs`) and by the
  editor (`settings/data/pick.ts`), both tested on the same game log; several files with the same value are tested for
  the newest (by change time), every different value and none (empty, or the text it says). The short form `{name}` is
  tested beside `{{name}}`, with Jira's own names, numbers, JSON and spaces left as written, and a field fill reading
  `{app.title}`. The Settings page is tested selecting a value in a log, refusing a name already used, settling
  several files the same way everywhere and keeping a value from 0.4 working. Selecting by click and by drag, the
  search, ticking files without the panel moving, the values list and the builder's `{` were looked at in a browser
  with demo files, not yet with a real capture.
- **The 10 November round:** unit tests for the recording sign's geometry and, on a virtual X11 display, the real
  windows (`core/indicator/src/x11.rs`); the Windows sign and the Windows text reader are compiled by Windows CI but
  have not run on a real PC. *Read text* is tested with tesseract on two pictures (light on dark, small text) for each
  of its three ways; tables and code blocks per service (`richtext_tests.rs`), pictures in the text for Jira and
  GitHub against their mock servers; the table grid, the values list, the column control and the Read text panel
  against the in-memory stand-in, and looked at in a browser with demo data.
- **Linux end to end** (virtual X11 display): the shortcut captures a fake program, the CargoFrame
  window opens with the capture, closing and reopening behaves as each of the three memory modes
  promises, and the real release binary stays at 19.6 MB of its own memory while idle
  (`scripts/e2e/`).
- **The windows in a browser** with demo data: the layout builder, Connections page, the share
  flow, themes and keyboard use (checked by hand-driven scripts, not kept as tests). For the
  settings update: the new General page in light and dark, the Save locally button and result,
  and the template panel. The real program (virtual display) was also driven to the Layouts page
  to see the template list come from the backend, and the capture end-to-end check still passes.
- **Connectors against local mock servers:** Jira (Cloud and Server), Google (Drive, Sheets),
  Reddit, Claude hand-off, Custom API, OAuth with PKCE, retry and fallback zip. The mock servers
  follow each service's documented API; they are not the services.
- **Installers:** GitHub's Windows runner builds the NSIS installer and runs a silent install,
  upgrade and uninstall; GitHub's Ubuntu runner builds the AppImage, installs it, runs it and
  quits it. See `.github/workflows/bundle.yml`. The Windows runner also opens the real installer
  window, checks that its one button says *Install*, presses it, and checks that the window closes
  by itself, the program is installed and CargoFrame opens its Settings window
  (`installers/windows/click-test.ps1`; screenshots of each step are kept as an artifact).
- **The Setup page** (the programs CargoFrame uses and the one install button): every sentence it
  can show, the winget command's fixed arguments, one install at a time, and the first-run marker
  are unit tests; the page's states (nothing missing, Windows with ffmpeg missing, install running,
  install failed, no winget, no loopback device, Linux command to copy) are tested against the
  in-memory stand-in.
- **Security review** with fixes and regression tests: `docs/security.md`.
- **Performance budgets** as CI gates with measured numbers: `docs/performance.md`.

## What is not verified

- **The settings update on a real desktop.** Save locally into a real folder, export and import
  through the system file dialogs, and the text and log rules on real log files have been tested
  by unit tests and the in-memory stand-in; the file dialogs themselves were not opened.
- **The 7 November round on a real desktop.** The default sound output is read by a PowerShell probe that has only been
  unit-tested and run on Windows CI; a real Discord webhook, the forum thread title convention, the Claude desktop
  app install through winget, and the picture of a frame taken from a playing clip in WebView2 (the screenshot at the
  end of a recording) have not been tried on real hardware or with real accounts. Slack has no webhook mode (a Slack
  webhook cannot attach files). Google, Reddit and Teams still need an app ID you register yourself.
- **The second 7 November round on a real PC.** The user has run the installer ("works well"). Not yet seen on real
  hardware: the Windows sound capture (everything, and one program's sound; the program is found from the window in
  focus), a Discord thread post through a real webhook (the earlier failure could not be reproduced here; the cause found
  is a thread sent through another channel's webhook), and the thread list of a bot token. Slack still needs a bot token.
- **A real desktop.** Windows 11 capture and tray, GNOME and KDE trays, Wayland capture and
  clipboard, notification clicks: spikes S1 to S5 in `docs/spikes.md` are all still "not run".
- **Real accounts.** OAuth sign-in with Google and Reddit, Jira tokens, and a Claude chat opened
  from a link have only been exercised against mock servers or by unit test. Spike S5 (how
  Claude's composer treats a pasted image) is open.
- **The Linux credential store** (Secret Service) is covered by an in-memory stand-in in tests,
  not by GNOME Keyring or KWallet.
- **Performance on a reference machine** (spike S6), including the 40 MB tray-only budget and the
  cold-open times, which were slightly over budget on a virtual display.
- **The Setup page on a real PC.** The winget install of ffmpeg has never been run (the Windows
  runner does not install it), nor has the Stereo Mix check or the button that opens the Windows
  sound settings. Whether winget is present, and whether it is allowed to install per user on a
  managed PC, is not known.
- **How the one-button installer looks.** Its structure is tested; how the window looks on a real
  screen has not been seen by a person.
- **Signing and releases.** `release.yml` and `scripts/windows/sign.ps1` have never run with a
  certificate; the program inside the installer is not signed. `new-signing-material.ps1` has
  not been run.
- **Wayland on real desktops.** Verified on a headless sway desktop with the portal, PipeWire and
  XWayland (`scripts/e2e/wayland-capture.sh`, also run as a GNOME-like desktop): shortcuts through
  sway and through GNOME's keyboard settings (real gsettings), the Wayland and XWayland program in
  focus, `grim` and portal screenshots, the uninstaller removing GNOME shortcuts. Checked against
  fakes that follow the documented D-Bus interfaces: the KWin script (KDE) and the portal's
  GlobalShortcuts. Recording: the portal session was asked of the real wlr portal (which could not
  copy frames on a GPU-less headless screen), and GStreamer reading a real PipeWire video node into
  a real ffmpeg recording works. Not yet seen: a real GNOME, KDE or Hyprland session, KDE's shortcut
  dialog, GNOME's screen-sharing dialog and its remembered choice, and `wtype` pasting into a browser.

## Phases

| Phase | Scope | State |
|-------|-------|-------|
| 0 Foundations | Workspace, CI, signing docs, spike runbook | Done. Owner runs: signing material, spikes, name check |
| 1 Shell and installer | Tray, single instance, launch notice, Settings window, installers | Done and built in CI |
| 2 Capture and window | Triggers, capture, CargoFrame window, themes | Done; Windows, X11 and Wayland (sway, Hyprland, KDE, GNOME through XWayland and the portal); Wayland verified on headless sway |
| 3 Builder and safe settings | Layout builder, data rules, zip, safe settings | Done |
| 4 Connectors | Jira, Google, Reddit, Claude, Slack, Discord, Teams, Custom API, Connections page, share flow | Done against mock servers |
| 5 Hardening and release | Security review, performance gates, release workflow, docs | Done in code; first real release run and reference-machine measurements are open |
| 12 Video (in progress) | `core/video`: recorder, ring of segments, edit model, export, tested with a real ffmpeg on a virtual screen. A layout Video block (record the screen or the window in focus; the computer's sound and a microphone), up to ten CargoFrame windows at once, ready-made video layouts (save locally, Jira, Drive, Slack), a trigger that makes a video (press to start, press to stop), the clip played, cut (timeline with handles), cropped, marked (a ring with a text) and sent from the CargoFrame window. Settings, Recording: find ffmpeg, choose capture and encoder, a 5-second test recording watched in the window (cutting, cropping and marking are in the CargoFrame window only, since 8 Nov) | Capture on Windows and Wayland waits for spike S9 (the test recording is how you run it); per-connection size hints and marks that come and go at chosen moments are not built; sound of the computer on Windows is recorded by CargoFrame itself (everything, or the program in focus; not yet tried on a real PC) |

## What the owner needs to do before calling V1 solid

1. Run spikes S1 to S7 on real machines and write the answers into `docs/spikes.md`; change the
   named setting in code where an answer differs from the assumption.
2. Use it with real accounts: one Jira (Cloud or Server), one Reddit, one Google, one Claude chat,
   and one workspace each for Slack, Discord and Teams (a Teams app registration needs a work or
   school account; see the form on the Connections page).
3. Run `scripts/windows/new-signing-material.ps1`, add the repository secrets it prints, and push
   a tag (`git tag v0.1.0 && git push origin v0.1.0`) to try the release workflow. It makes a
   **draft** release; nothing is published until you publish it.
4. Check the product name for conflicts (S7).
5. Reconcile your local copy of the repository with GitHub. Earlier files were delivered there by hand and may
   duplicate files now in git.
6. Confirm that "log action" in the brief means **data rule** (see `docs/decisions.md`).
   Video (spike S9), on Windows: install ffmpeg from Settings, Setup, open Settings, Recording, press
   "Test ffmpeg", then "Try a 5-second recording", then "Watch it here" and "Open the clip". To see
   what a long recording costs, record a minute with a video trigger while Task Manager is open.
   Try Capture = Desktop Duplication and an NVIDIA/Intel/AMD encoder if you have one. Write
   what worked and what each cost into the S9 table in `docs/spikes.md`.
   Recording: on the Triggers page make a trigger whose layout has a Video block and a sharing button (the form
   says the press records), press its shortcut, do something on the screen, press it again; the window should open
   with the clip playing. Send it to Slack, Google Drive, Jira and Discord and write down which size limits
   you hit; try Reddit/Teams/Claude to see the reason shown. Press the shortcut once and use the tray's "Recording: stop and
   discard it". In the window, drag the two handles of the thin timeline under the clip (or use the arrow keys),
   hover the picture and try the play, stop, slower, faster, volume and full-screen buttons (full screen is
   untested in WebView2 and WebKitGTK), press "Crop", drag the box and its corner and press "Apply crop" (the window
   must then show the cropped picture), "Mark", click where the ring goes and type a text, then "Reset to full capture", send, and check that
   what arrives is only the kept part and the kept area, with the ring and its text where you put them.
   Tick boxes: with two or three sharing buttons in a layout (Jira, Claude, Slack), look at the boxes on the top
   edge of each block, clear one and send, and check that button left that block out and the others did not; hover a
   button and check its blocks light up; in the layout builder, choose where a text block goes from a button's list.
   Windows, window and sound: press shortcuts of two triggers one after the other and check that two CargoFrame windows
   are open, each sending only its own capture (try ten, then an eleventh: a notification says there is no room);
   in a Video block choose *The window in focus*, record a clip while a smaller window is in front and check that only
   that window is in it (also with a second screen, a window half off the screen, and Windows display scaling at 150%,
   where ffmpeg's GDI capture may come out the wrong size); turn the computer's sound on and play something while you
   record (Linux: the default output; Windows: needs Stereo Mix or another loopback device), switch the microphone on
   and speak, and check both are in the clip and in what is sent; remove the loopback device and check the notification
   says why there is no computer sound.
   The second 7 November round, on a real PC: record a video with a video playing and check the sound is in it (Sound
   from: everything, then The program in focus); in Setup press the sound button and check it opens the **Playback** tab;
   in a Discord layout choose a channel, then a thread, send, press the shortcut again and check it starts on the same
   thread, then pick a forum and check the posts are listed (bot) or the last ones used (webhook); type in the Title and
   check the thread list narrows (and *Show all* undoes it); in General switch both notice kinds off, record, and check
   nothing pops up; open *Your data*, clear one kind and then press *Reset CargoFrame* on a spare install; in a text block
   use the formatting buttons and send to Jira, Slack and Discord and look at how each shows bold, a list and code.
   The Settings window since 8 Nov: check the sidebar groups (Get started, Captures, CargoFrame), that General,
   Your data and About each hold what their name says, and that an update notice opens About. On Linux behind a
   company proxy that inspects traffic, check that a connection's Test works (the system's certificates are trusted).
   Look: open Settings, General, Appearance, pick an accent (try Orange and a very light one), and check that the
   buttons, the selection, the logo arrow and the tray icon's arrow all change, in both themes. Look at the icons
   (program, installer, tray) on a light and a dark taskbar.
   Installer and Setup: download `CargoFrame-Setup.exe` from the release, double-click it and press
   Install (check that nothing else is asked, that CargoFrame opens on the Setup page, and that
   Windows' "unknown publisher" notice can be passed with More info, Run anyway). On the Setup page
   press *Install ffmpeg* and check that it finds ffmpeg by itself afterwards (no sign-out), then
   Settings, Recording, *Test ffmpeg*. Turn off every sound device but the microphone and check the
   Stereo Mix line and the button that opens the sound settings. Run the installer again over the
   installed copy: the button says Update and your settings are kept.
   The 7 November round, on a real PC: on the Setup page look at the *Sound output* line (does it name your
   speakers; does it say videos can include the sound); add a Discord connection with one webhook link, make a
   layout from the Discord template and send a message, then a forum thread by title; add a second channel and a
   category in a bot-mode connection and check the dependent dropdowns narrow (server, category, channel); send twice
   to different channels and check the dropdown starts on the last one and lists the last five; press the buttons
   next to the token fields and check each opens the right page; from a Claude connection press *Install it* for the desktop
   app; in a layout with both a Screenshot and a Video block, record, then press *Take screenshot* and send to
   Jira or Slack and check both arrive; open **Guides** at the bottom of the Settings menu, run the Jira guide and one
   other, and check the box lands on the right field and the sentence is short.
7. Before making the repository public: the list at the end of `docs/signing.md` and in
   `docs/security.md`.

## Decided

- **Twitter/X, BlueSky and Instagram:** v1.1 (2026-10-23).
- **Repository:** private until V1 is solid. "Check for updates" works with a read-only GitHub token you create; the automatic daily check is off by default.
- **Still to confirm:** "log action" is read as "data rule".
