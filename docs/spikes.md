# Phase 0 spikes

Each spike answers one question that the rest of the plan depends on. Run them on real
machines, fill in the result, and change the named setting in code if the answer differs from
the assumption. Targets here are proposals until a spike confirms them.

| # | Question | Needs | Result |
|---|----------|-------|--------|
| S1 | Does the per-user installer behave as planned? | Windows 11 VM | _not run_ |
| S2 | Can a notification click open Settings? | Windows 11, GNOME, KDE | _not run_ |
| S3 | Do global shortcuts and capture work on Wayland? | GNOME and KDE, Wayland | _not run_ |
| S4 | Does the tray icon show on GNOME and KDE? | GNOME, KDE | _not run_ |
| S5 | How does Claude open from a link, and what does its composer do with a pasted image? | Windows 11 with the Claude app; a Linux desktop | _not run_ |
| S6 | Are the performance budgets reachable? | One reference machine | _not run_ |
| S7 | Is "CargoFrame" free to use as a product name? | Web | _not run_ |

## S1. Per-user installer (Windows)

**Question.** Does the Tauri NSIS bundler in current-user mode install to
`%LocalAppData%\CargoFrame` without an administrator prompt, and does running a newer installer
upgrade in place?

**Steps.**
1. Build two installers with different versions (`bundle.windows.nsis.installMode = "currentUser"`).
2. On a clean Windows 11 VM with a standard user, run installer 1. Note: any UAC prompt, install
   folder, Start Menu shortcut, registry keys (should all be under `HKCU`).
3. Change a setting, tick "run on sign-in", run installer 2 while the app is running.
4. Uninstall; check the "delete settings?" question and that `%AppData%\CargoFrame` stays when
   the answer is no.

**Pass.** No UAC prompt. Same install folder after the upgrade. The running app quits cleanly
(single-instance channel, not a kill). Settings and the startup choice survive. If the folder
cannot be pinned with the stock template, use an NSIS hook (`installerHooks`).

## S2. Notification click opens Settings

**Question.** Can clicking the "CargoFrame is running" notification open the Settings window?

**Steps.**
- Windows: the Start Menu shortcut must carry the app's AppUserModelID. Try a toast API that
  reports activation (for example `tauri-winrt-notification` with `on_activated`) from the
  installed app, not from `cargo run`.
- Linux: send the notification with a `default` action (`notify-rust`) and wait for the action
  on GNOME and KDE.

**Pass.** Click opens Settings in under 500 ms. **Fallback.** The tray icon stays the guaranteed
route; the notification text then says "Click the tray icon to edit settings."

## S3. Wayland: shortcuts and capture

**Question.** Which trigger and capture modes work on GNOME and KDE under Wayland?

**Steps.** Register a global shortcut through the `org.freedesktop.portal.GlobalShortcuts`
portal and capture through `org.freedesktop.portal.Screenshot` or ScreenCast. Record: permission
prompts (every time or remembered), which capture modes are possible (full screen, region, active
window), and the delay from key press to image.

**Expected limit (confirm).** Wayland does not let an app read another app's window, so the
"active window" mode is likely unavailable there and falls back to full screen or an interactive
portal selection. The settings page should say so rather than offer a mode that cannot work.

## S4. Tray on GNOME and KDE

**Question.** Does the tray icon appear, and what do we do when there is no tray host?

**Steps.** Run a minimal Tauri tray app on KDE Plasma, on GNOME with the AppIndicator extension,
and on stock GNOME. Check for the `org.kde.StatusNotifierWatcher` name on the session bus at
startup.

**Pass.** Icon and left-click work on KDE and on GNOME with the extension. With no host, the app
opens Settings at launch with a one-line hint about the extension.

## S5. Claude link and clipboard

**Status: settled by the documentation, then replaced by pasting.** Claude's own help says a link
can only open a chat: `claude://claude.ai/new?q=` fills a new chat with a short text, and
`claude://claude.ai/chat/<id>` opens an existing one with nothing filled in. No link attaches a
picture or a file. So a share to Claude no longer relies on links to carry anything. It opens the
chat, waits until the Claude window (or a browser showing Claude) is really in front, pastes the
screenshot and the ticked files as files, pastes the message, and presses Enter
(`core/deliver`). See `docs/decisions.md`.

**What is still worth checking on a new machine** (Windows 11 with the Claude desktop app, then a
browser):
1. Does the link open the chat and bring the window to the front within a few seconds? If not,
   raise **Extra seconds to wait before pasting** in the Claude connection.
2. Does pasting several files from the clipboard attach all of them? (Claude takes at most 20
   files of 30 MB each in one message.)
3. Does Enter send once the files have finished uploading? If it fires too early, raise the extra
   seconds.
4. Linux: there is no `claude://` handler, so the browser is used; pressing keys needs X11 and
   `xdotool`, and files cannot be pasted yet (the message is, and a note says so).

**Settings this decides:** in the Claude connection: **Open Claude with**, **Send the message for
me (press Enter)** and **Extra seconds to wait before pasting**.

## S6. Performance budgets

Measure on one reference machine (write down the CPU, RAM, GPU and display setup) with `tracing`
spans and the debug panel.

| Measure | Target | Measured |
|---------|--------|----------|
| Tray-only memory (no webview open) | under 40 MB | |
| Idle CPU | about 0% | |
| Trigger to captured image (one 4K display) | under 150 ms | |
| Trigger to usable window, warm | under 250 ms | |
| Trigger to usable window, cold | under 700 ms | |
| Settings window, cold open | under 500 ms | |
| UI while dragging blocks | 60 fps | |
| JavaScript per window, gzipped | under 250 KB | |

CI fails a build that misses a target by more than 20%.

## S7. Name check

Search for "CargoFrame" as a product, company, trademark (USPTO, EUIPO) and domain before icons,
installer names and the app ID are fixed. Record the date and what was found.
