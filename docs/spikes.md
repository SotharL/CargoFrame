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
| S9 | What does a screen recording cost, and which capture and encoder should video use? | Windows 11 and a Linux desktop (X11 and Wayland), ffmpeg | _not run_ |

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

## S9. Video: capture and encoding

**Question.** How should CargoFrame record the screen, and what does a 10 s clip cost in CPU and
memory on Windows 11 and on Linux? (Plan, Phase 12.)

**Proposed answer to check.** Recording starts on the shortcut. ffmpeg runs as a child process
that captures the screen and encodes as it goes, writing one-second segments into a temporary
folder and listing each finished one; CargoFrame keeps only the newest ones (10 s by default, 30 s
at most) and deletes the rest, so the buffer is a ring of *encoded* files on disk, not frames in
memory. When recording stops the last segments are joined into the clip, which goes to the editor.
Why: raw frames cannot be buffered (10 s of 1080p is about 2.5 GB, of 4K about 10 GB), the
encoder has to run while recording, and a child process keeps every frame out of CargoFrame's own
memory and keeps `unsafe_code = "forbid"` (no capture API calls in the app). The pieces that do
not depend on this answer are already built and tested: the rolling buffer, the edit model and
the ffmpeg export (`core/video`).

**Quickest check (Windows or Linux):** open Settings, Video, press "Test ffmpeg" and then "Try a
5-second recording". It uses the same recorder the product will, reports whether it worked, and
keeps the clip so you can watch it ("Watch it here" also tells whether the Settings window itself
can play H.264, which decides how the editor shows the clip). Change Capture and Compression and
try again to compare;
the scripts below measure CPU and memory as well.

**What to run.**
- Windows: `powershell -ExecutionPolicy Bypass -File scripts\windows\s9-video.ps1` (needs ffmpeg:
  `winget install Gyan.FFmpeg`). It records 10 s with each of `gdigrab` and `ddagrab` and each
  encoder it finds (software x264, NVIDIA, Intel, AMD, Windows built-in) and writes `s9-video.log`.
  Move windows around while it runs, and watch Task Manager, Performance, GPU ("Video Encode",
  "3D") during each run; GPU load is not measured by the script.
- Linux, X11 session: `scripts/e2e/s9-video-linux.sh` (needs ffmpeg and python3). It writes
  `s9-video-linux.log`. Without a display it records a virtual one, which only proves the script.
- Linux, Wayland session: x11grab cannot see Wayland windows, so the script is no help. Record
  with the desktop's own recorder (GNOME: Ctrl+Alt+Shift+R) or OBS for 10 s at the same size and
  read CPU and memory from `top`/System Monitor, and note whether ffmpeg in your distribution can
  read the portal's PipeWire stream (`ffmpeg -devices | grep -i pipewire`; ffmpeg 7.1 and later
  can). If it cannot, the Wayland path needs GStreamer (`pipewiresrc`) instead, which changes the
  plan for Linux and should be written down here.

**Measure** (one reference machine; write down CPU, RAM, GPU, display size and scaling):

| Measure | Target (proposal) | Windows | Linux X11 | Linux Wayland |
|---------|-------------------|---------|-----------|---------------|
| CPU while recording 1080p30, best encoder | under 10% of the machine | | | |
| CPU while recording 1080p30, software x264 | under 25% of the machine | | | |
| Same on the largest display, downscaled to 1080p | no worse than above | | | |
| Peak memory of the recorder process | under 300 MB | | | |
| Clip size for 10 s of a busy screen | under 25 MB (Discord's limit is 10 to 25 MB) | | | |
| Dropped or duplicated frames | under 2% | | | |
| Start of recording to first frame | under 500 ms | | | |
| Stop to editor open (join the segments) | under 1 s | | | |
| Idle CargoFrame while *not* recording | unchanged (about 0%) | | | |

**Also answer.**
1. Does the hardware encoder of each vendor work through ffmpeg on your machine, and does it cost
   clearly less than software x264? (Decides the default of the *Encoder* setting: automatic,
   hardware first.)
2. Does `ddagrab` beat `gdigrab` enough to be the default on Windows? Does it work on a laptop
   with two GPUs, and on a second display?
3. Is the picture sharp enough at the CRF used (23 for veryfast, 28 for ultrafast) for small text?
4. Licence and size: ship ffmpeg with CargoFrame (an LGPL build is about 80 MB on disk, 30 MB
   compressed) or ask the person to install it? Recommendation until measured: ask the person to
   install it, with the location settable, because a bundled copy doubles the installer; revisit
   once the clip works end to end.

**Settings this decides:** *Encoder* (automatic, software, or one named), *Frame rate* (30),
*Largest picture* (downscale above 1080p or not), the capture method per platform, and whether
Linux Wayland has video in version 1.

