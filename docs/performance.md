# Performance

CargoFrame is meant to sit in the tray for days and cost almost nothing, then feel instant when
the shortcut is pressed. This page lists the budgets, how each one is checked, and what has
actually been measured. It is honest about what has **not** been measured on a real computer yet.

## Budgets

| What | Budget | CI gate |
|------|--------|---------|
| Tray-only process, memory | under 40 MB | anonymous memory ≤ budget +20% (Linux); private bytes reported (Windows) |
| Tray-only process, CPU while idle | about 0% | ≤ 2% over 10 s |
| Shortcut → capture complete (4K screen) | under 150 ms | the picture is ready to show in ≤ 120 ms +20% (release build) |
| Crop of a 4K picture | under 20 ms | ≤ 20 ms +20% |
| Shortcut → usable CargoFrame window | under 250 ms warm, under 700 ms cold | measured on a virtual display, ≤ budget +20% |
| Settings window, cold | under 500 ms | ≤ budget +20% |
| JavaScript each window loads up front | 250 KB gzip | ≤ 250 KB +20% (`scripts/ci/check-budgets.mjs`) |

"+20%" is the margin the plan allows for a noisy machine. Shared CI runners are slower than a
desktop, so the timing steps run with `CARGOFRAME_PERF_SCALE=2`, which doubles every time budget.
That keeps them useful as a tripwire for a real regression (10× slower, a blocking call on the
wrong thread) without failing on a busy runner.

## How each one is enforced

| Check | Where | Runs |
|-------|-------|------|
| JavaScript size per window | `scripts/ci/check-budgets.mjs` | `ci.yml`, UI job, after the build |
| Capture picture and crop timing | `core/capture/tests/perf.rs` (`#[ignore]`, release only) | `ci.yml`, Rust job, Linux and Windows |
| Idle memory and CPU of the tray-only process | `scripts/e2e/linux-idle.sh` | `bundle.yml`, Linux, release binary |
| Window open times (cold, warm, Settings) | `scripts/e2e/linux-open-times.sh` | `bundle.yml`, Linux, release binary |
| Idle memory and CPU on Windows | `installers/windows/smoke-test.ps1` | `bundle.yml`, Windows, installed program (CPU gated, memory reported) |

While running, the program also times itself (`app/src/timing.rs`): from the shortcut to the
capture being ready, to the window being shown, and to the first painted frame. Settings, General,
Performance shows the last numbers. Timings are kept in memory and in the log as plain
milliseconds. They contain no window titles, paths or content, and nothing is sent anywhere.

## Memory modes

Settings, General, Performance, Memory decides what a closed window costs.

| Mode | A closed window | Reopening |
|------|-----------------|-----------|
| Low memory | destroyed at once, web view freed | cold start |
| Balanced (default) | hidden for 5 minutes, then destroyed | instant within 5 minutes, cold after |
| Always warm | hidden and kept | instant |

The tray-only process never holds a web view, whatever the mode.

## Measured so far

Measured on the cloud build machine (Linux, virtual display `xvfb`, software rendering, release
build), last re-run on 7 Nov 2026 after the feedback round. These are the numbers the gates were set against.
The settings window's script grew from 52 to 94 KB with the video pages, the guides and the Discord form; both windows stay far under the 250 KB budget.

| Measurement | Result |
|-------------|--------|
| 4K screenshot to PNG, fast setting | 96 ms (11.7 MB for a screenshot-like picture) |
| Crop 1920×1080 out of 4K | 1.7 ms |
| Shortcut → capture ready (small window) | 3 ms |
| CargoFrame window usable, cold | 736 ms |
| CargoFrame window usable, warm (Balanced, reopened) | 36 ms |
| Settings window open, cold | 686 ms |
| Tray-only process, anonymous memory | 19.7 MB |
| Tray-only process, resident set / proportional set | 100 MB / 79 MB |
| Tray-only process, CPU over the idle window | 0.13% |
| JavaScript up front, CargoFrame window | 52 KB gzip |
| JavaScript up front, Settings window | 94 KB gzip |

## What these numbers do not prove

- **Memory.** The resident set looks large because the process maps the Mesa software renderer
  (an LLVM library) and WebKit's shared libraries as soon as the toolkit opens a display. Those
  pages are shared with other programs and are not CargoFrame's own cost, which is why the
  Linux gate measures anonymous memory and reports the other two. On a real desktop with a GPU
  driver the picture will differ. **The 40 MB budget still has to be confirmed on a reference
  machine** (spike S6 in `docs/spikes.md`), on Windows 11 and on one GNOME and one KDE desktop.
- **Cold open times.** 736 ms and 684 ms are slightly over the 700 ms and 500 ms budgets on a
  virtual display with software rendering, which is why CI uses a scale of 2. A real GPU should
  be faster, but that is an expectation, not a measurement.
- **Windows.** Capture, window and idle timings on Windows 11 have only the smoke test behind
  them. Nothing was timed on a real Windows desktop.
- **Dragging blocks at 60 fps** in the layout editor was not measured. It was checked by eye in
  a browser, with no animations in the way.
- **A very large capture** (several 4K displays at once) was not timed. Only one display or one
  window is captured at a time.

## If a gate fails

1. Read the failing line: each gate prints the measured time next to its budget.
2. Run the same script on your own computer. If it passes there with `CARGOFRAME_PERF_SCALE=1`,
   the runner was just slow: re-run before changing anything.
3. If it fails everywhere, the change is the cause. Do not raise a budget to make a build pass;
   change the plan first (`docs/decisions.md`) so the new number is a decision, not drift.
