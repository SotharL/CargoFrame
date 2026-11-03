# Status

_Last updated 2026-11-03 (phases 8 to 12, including video, were merged to `main` in pull request #19; the rest dates from 2026-10-23). V1 was merged to `main` from `dev/v1` in pull request #1. The settings update (General and Safety, Save locally, layout templates, text and log rules, export and import) is in pull request #12._

**V1 is feature-complete in code. It has not yet run on a real Windows 11 or Linux desktop, and
no real Jira, Google, Reddit or Claude account has been used.** Everything below says which of
those is true for each piece.

## What is verified

- **Unit and integration tests:** 743 Rust tests and 272 UI tests pass. `cargo clippy -D warnings`,
  `cargo fmt --check` and `svelte-check` are clean.
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
  quits it. See `.github/workflows/bundle.yml`.
- **Security review** with fixes and regression tests: `docs/security.md`.
- **Performance budgets** as CI gates with measured numbers: `docs/performance.md`.

## What is not verified

- **The settings update on a real desktop.** Save locally into a real folder, export and import
  through the system file dialogs, and the text and log rules on real log files have been tested
  by unit tests and the in-memory stand-in; the file dialogs themselves were not opened.
- **A real desktop.** Windows 11 capture and tray, GNOME and KDE trays, Wayland capture and
  clipboard, notification clicks: spikes S1 to S5 in `docs/spikes.md` are all still "not run".
- **Real accounts.** OAuth sign-in with Google and Reddit, Jira tokens, and a Claude chat opened
  from a link have only been exercised against mock servers or by unit test. Spike S5 (how
  Claude's composer treats a pasted image) is open.
- **The Linux credential store** (Secret Service) is covered by an in-memory stand-in in tests,
  not by GNOME Keyring or KWallet.
- **Performance on a reference machine** (spike S6), including the 40 MB tray-only budget and the
  cold-open times, which were slightly over budget on a virtual display.
- **Signing and releases.** `release.yml` and `scripts/windows/sign.ps1` have never run with a
  certificate; the program inside the installer is not signed. `new-signing-material.ps1` has
  not been run.
- **Wayland "active window" capture** may not be possible on some compositors; the fallback is
  the display under the cursor.

## Phases

| Phase | Scope | State |
|-------|-------|-------|
| 0 Foundations | Workspace, CI, signing docs, spike runbook | Done. Owner runs: signing material, spikes, name check |
| 1 Shell and installer | Tray, single instance, launch notice, Settings window, installers | Done and built in CI |
| 2 Capture and window | Triggers, capture, CargoFrame window, themes | Done; Windows, X11 and the Wayland portal are implemented, Wayland is untested |
| 3 Builder and safe settings | Layout builder, data rules, zip, safe settings | Done |
| 4 Connectors | Jira, Google, Reddit, Claude, Slack, Discord, Teams, Custom API, Connections page, share flow | Done against mock servers |
| 5 Hardening and release | Security review, performance gates, release workflow, docs | Done in code; first real release run and reference-machine measurements are open |
| 12 Video (in progress) | `core/video`: recorder, ring of segments, edit model, export, tested with a real ffmpeg on a virtual screen. A layout Video block, ready-made video layouts (save locally, Jira, Drive, Slack), a trigger that makes a video (press to start, press to stop), the clip played, cut (start and end), cropped, marked (a ring with a text) and sent from the CargoFrame window. Settings, Video: find ffmpeg, choose capture and encoder, a 5-second test recording, record until stopped, watch the clip in the window, export a trimmed/cropped/marked copy (marks drawn by ffmpeg or by the window), edited on the picture: timeline with handles, crop box, a click to place the mark | Capture on Windows and Wayland waits for spike S9 (the test recording is how you run it); per-connection size hints and marks that come and go at chosen moments are not built |

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
   Video (spike S9), on Windows: install ffmpeg (`winget install Gyan.FFmpeg`), open Settings,
   Video, press "Test ffmpeg", then "Try a 5-second recording", then "Watch it here" and "Open the
   clip". Try "Record until I stop" for a minute with Task Manager open, and "Trim, crop and mark a
   copy" with the drawn mark (look at how the text comes out). While the clip plays, drag the two
   handles of the timeline under it, drag the crop box, and click on the picture to place the drawn
   mark and give it a text. Try Capture = Desktop Duplication and an NVIDIA/Intel/AMD encoder if you have one. Write
   what worked and what each cost into the S9 table in `docs/spikes.md`.
   Video trigger: on the Triggers page make a trigger with Makes = Video (and a layout with a Video block and a
   sharing button), press its shortcut, do something on the screen, press it again; the window should open
   with the clip playing. Send it to Slack, Google Drive, Jira and Discord and write down which size limits
   you hit; try Reddit/Teams/Claude to see the reason shown. Press the shortcut once and use the tray's "Recording: stop and
   discard it". In the window, drag the two handles under the clip (or use the arrow keys),
   press "Start here" / "End here" while it plays, tick "Crop" and drag the box and its corner, tick "Mark", click where the ring goes and type a text, send, and check that
   what arrives is only the kept part and the kept area, with the ring and its text where you put them.
   Look: open Settings, General, Appearance, pick an accent (try Orange and a very light one), and check that the
   buttons, the selection, the logo arrow and the tray icon's arrow all change, in both themes. Look at the icons
   (program, installer, tray) on a light and a dark taskbar.
7. Before making the repository public: the list at the end of `docs/signing.md` and in
   `docs/security.md`.

## Decided

- **Twitter/X, BlueSky and Instagram:** v1.1 (2026-10-23).
- **Repository:** private until V1 is solid. "Check for updates" works with a read-only GitHub token you create; the automatic daily check is off by default.
- **Still to confirm:** "log action" is read as "data rule".
