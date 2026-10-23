# Status

_Last updated 2026-10-23._

## What is verified

- `cargo test --workspace`: **91 tests pass** (context 31, handoff 14, layout 21, safe 25).
- `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --check` are clean.
- Run in a Linux workspace without network access to crates.io, so the crates have **no
  dependencies**.

## What is not verified

- The GitHub Actions workflow has not run on GitHub yet. Expect to fix small things on its first run.
- `scripts/windows/new-signing-material.ps1` has not been run (no PowerShell in the workspace).
- Nothing that needs a dependency has been written or compiled: Tauri shell, tray, installer,
  capture, JSON config, connectors, Svelte UI.

## Phases

| Phase | Scope | State |
|-------|-------|-------|
| 0 Foundations | Workspace, CI, signing docs and script, spike runbook | Scaffold, CI and docs done. **You** run: signing material, spikes S1 to S7, name check |
| 1 Shell and installer | Tray, single instance, launch notice, Settings window, NSIS and Linux installers | Not started (needs Tauri dependencies) |
| 2 Capture and window | Triggers, capture, CargoFrame window, themes | Folder climb, templates, allow-list and layout model done; capture and UI not started |
| 3 Builder and safe settings | Layout builder, data rules, zip, safe settings | Safe-settings retry and ladder, layout validation and grid done; zip, parsers, builder UI not started |
| 4 Connectors | Jira, Google, Reddit, Claude, Custom API | Claude link and chat-list logic done; connectors not started |
| 5 Hardening and release | Performance gates, security review, release workflow | Not started |

## Blockers for the rest of V1

1. **This build session cannot reach the repository on GitHub.** It is private and the session's
   GitHub credential has no access to it. Everything above was written into a local copy and
   delivered to your clone as files; nothing was committed or pushed.
2. **This build session cannot download packages.** The workspace's network policy refused
   `index.crates.io` and `registry.npmjs.org` (403 from the proxy), and the Ubuntu package
   servers were unreachable. Compiling the Tauri shell needs the crates, the webkit/GTK
   development packages and, for the UI, npm.

To unblock:

- **Network:** in the Claude app, open the session, select the caret after the session title,
  choose **Edit cloud environment**, and set **Network access** to **Trusted** (the default level,
  which already allows crates.io, npm, the Ubuntu archive and GitHub). The 403s on
  `index.crates.io` and `registry.npmjs.org` suggest the environment is set to **None** or to a
  **Custom** list without the defaults. If the dialog is read-only, the environment is shared by
  an organization and an Owner changes it under Cloud environments in admin settings.
- **GitHub:** reconnect it under claude.ai Settings > Connectors and make sure the repository is
  included.

## Decided

- **Twitter/X and BlueSky:** v1.1, both (2026-10-23).
- **Still to confirm:** "log action" is read as "data rule".
