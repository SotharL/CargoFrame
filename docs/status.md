# Status

_Last updated 2026-11-08, version 0.2.0 (a shortcut records or takes a screenshot from its layout; fixed: a window whose layout had a dropdown stayed on Loading; X, Bluesky and Instagram against mock servers, one build switch per service, `cargoframe open`; the flow review: Settings grouped by content, General split into General, Your data and About, the duplicate video editor removed from Settings, Linux trusts the system's certificates; before that, 2026-11-07, evening: the second 7 November round after the installer test: one path to post with threads, sound recorded from Windows itself, Your data and reset, notifications switch, text formatting, Setup checks, Data from files as a sentence; before that, the twelve changes of the first 7 November feedback round: Discord webhooks, dependent dropdowns, last choices, get-token buttons, Claude app install, screenshot with video, in-app guides, Connections under Setup; before that, phases 8 to 12, including video, were merged to `main` in pull request #19; the rest dates from 2026-10-23). V1 was merged to `main` from `dev/v1` in pull request #1. The settings update (General and Safety, Save locally, layout templates, text and log rules, export and import) is in pull request #12._

**V1 is feature-complete in code. It has not yet run on a real Windows 11 or Linux desktop, and
no real Jira, Google, Reddit or Claude account has been used.** Everything below says which of
those is true for each piece.

## What is verified

- **Unit and integration tests:** about 890 Rust tests and 499 UI tests pass. `cargo clippy -D warnings`,
  `cargo fmt --check` and `svelte-check` are clean.
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
5. Reconcile `C:\Users\Admin\Documents\GitHub\CargoFrame` with the repository. Earlier files were
   delivered there by hand and may duplicate files now in git.
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
