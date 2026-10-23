#!/usr/bin/env bash
# End-to-end check on a virtual X display: install a trigger and a layout, start CargoFrame, press
# the shortcut, and make sure the CargoFrame window opens showing the focused program's capture.
#
#   scripts/e2e/linux-capture.sh [path-to-cargoframe-binary] [output-dir]
#
# Needs: xvfb, xdotool, imagemagick (import), python3 with Pillow, dbus-run-session.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

if [ -z "${CARGOFRAME_E2E_INNER:-}" ]; then
  bin="${1:-$root/target/debug/cargoframe}"
  out="${2:-$root/target/e2e}"
  mkdir -p "$out"
  export CARGOFRAME_E2E_INNER=1 CARGOFRAME_E2E_BIN="$(readlink -f "$bin")" CARGOFRAME_E2E_OUT="$(readlink -f "$out")"
  exec dbus-run-session -- xvfb-run -a -s "-screen 0 1280x800x24" "$0"
fi

bin="$CARGOFRAME_E2E_BIN"; out="$CARGOFRAME_E2E_OUT"
fake="$root/target/debug/examples/fake_app"
[ -x "$fake" ] || { echo "build the helper first: cargo build -p cargoframe-capture --example fake_app" >&2; exit 1; }

work="$(mktemp -d)"
cleanup() {
  [ -n "${app_pid:-}" ] && kill "$app_pid" 2>/dev/null || true
  [ -n "${fake_pid:-}" ] && kill "$fake_pid" 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT
set -E
trap 'fail "a command failed (line $LINENO)"' ERR
fail() {
  echo "FAIL: $*" >&2
  if [ -f "$work/cargoframe.log" ]; then
    cp "$work/cargoframe.log" "$out/cargoframe.log" 2>/dev/null || true
    tail -n 40 "$work/cargoframe.log" >&2
  fi
  import -window root "$out/failure.png" 2>/dev/null || true
  exit 1
}

export HOME="$work/home"; mkdir -p "$HOME"
export CARGOFRAME_HOME="$work/cfg" CARGOFRAME_LOG=debug CARGOFRAME_LOG_STDERR=1
export WEBKIT_DISABLE_COMPOSITING_MODE=1 WEBKIT_DISABLE_DMABUF_RENDERER=1 GDK_BACKEND=x11

# The user's configuration: one trigger, one layout. Nothing else exists on a fresh install.
cfg="$CARGOFRAME_HOME/config"   # CARGOFRAME_HOME holds <home>/config and <home>/data
mkdir -p "$cfg/triggers" "$cfg/layouts"
cat > "$cfg/triggers/bug.json" <<'JSON'
{ "schema_version": 1, "name": "Bug report", "enabled": true, "shortcut": "Ctrl+Shift+F9",
  "capture": "active_window", "layout_id": "bug" }
JSON
cat > "$cfg/layouts/bug.json" <<'JSON'
{ "blocks": [
  { "id": "shot", "type": "screenshot", "rect": { "x": 0, "y": 0, "w": 8, "h": 9 } },
  { "id": "prompt", "type": "text", "rect": { "x": 8, "y": 0, "w": 4, "h": 3 },
    "title": "Prompt", "role": "prompt", "default_text": "What is wrong in {{project.name}}?" },
  { "id": "logs", "type": "files", "rect": { "x": 0, "y": 9, "w": 8, "h": 3 },
    "title": "Logs", "include": ["**/*.log"], "preselect": ["app.log"] },
  { "id": "send", "type": "sharing_button", "rect": { "x": 8, "y": 9, "w": 4, "h": 2 }, "label": "Share" }
] }
JSON

# The program in focus lives in <dev>/Foo/Foo, so its project is the folder Foo.
mkdir -p "$work/dev/Foo/logs"
cp "$fake" "$work/dev/Foo/Foo"
echo "started" > "$work/dev/Foo/app.log"
echo "debug"   > "$work/dev/Foo/logs/debug.log"
"$work/dev/Foo/Foo" "Foo - main.rs" > "$work/fake.out" 2>&1 &
fake_pid=$!
for _ in $(seq 50); do grep -q ready "$work/fake.out" 2>/dev/null && break; sleep 0.1; done
grep -q ready "$work/fake.out" || fail "the fake program did not start"

"$bin" --autostart > "$work/cargoframe.log" 2>&1 &
app_pid=$!
for _ in $(seq 100); do grep -qi "shortcut\|triggers" "$work/cargoframe.log" && break; sleep 0.1; done
sleep 0.5
kill -0 "$app_pid" 2>/dev/null || fail "CargoFrame exited at start-up"

# Keep the focus on the fake program, then press the shortcut as a person would.
fake_win="$(xdotool search --pid "$fake_pid" | head -n1)"
[ -n "$fake_win" ] || fail "no window for the fake program"
# There is no window manager here, so nothing composites the screen: raise the program above
# the Settings window that opens at launch when there is no tray, or the capture would show it.
xdotool windowraise "$fake_win" || true
xdotool windowfocus "$fake_win" || true
sleep 0.5
xdotool key --clearmodifiers ctrl+shift+F9

# Window titles match case-insensitively in xdotool, so compare the exact title ourselves.
find_window() {
  local w
  for w in $(xdotool search --name 'cargoframe' 2>/dev/null || true); do
    if [ "$(xdotool getwindowname "$w" 2>/dev/null)" = "$1" ]; then echo "$w"; return 0; fi
  done
  return 1
}
win=""
for _ in $(seq 100); do
  win="$(find_window CargoFrame || true)"
  [ -n "$win" ] && break
  sleep 0.1
done
[ -n "$win" ] || fail "the CargoFrame window did not open after the shortcut"
echo "ok: the CargoFrame window opened"

# Give the page time to load the session and draw the screenshot.
sleep 2.5
if [ -n "${CARGOFRAME_E2E_DEBUG:-}" ]; then
  for w in $(xdotool search --name '' 2>/dev/null); do
    echo "window $w: $(xdotool getwindowname "$w" 2>/dev/null) $(xdotool getwindowgeometry "$w" 2>/dev/null | tr '\n' ' ')"
  done
fi
# A cold web view paints late on a slow machine: look again for up to 20 seconds.
check_shot() {
python3 - "$out/capture-window.png" <<'PY'
import sys
from PIL import Image
im = Image.open(sys.argv[1]).convert("RGB")
w, h = im.size
colors = im.getcolors(maxcolors=w * h) or []
def has(rgb, tol=6, minimum=200):
    return sum(n for n, c in colors if all(abs(a - b) <= tol for a, b in zip(c, rgb))) >= minimum
# The captured program's red strip and blue bar must be visible inside the window.
if not has((0xE0, 0x4F, 0x4F)):
    sys.exit("the program's red strip is not on screen (capture not shown?)")
if not has((0x4F, 0x8E, 0xE0)):
    sys.exit("the program's blue bar is not on screen")
PY
}
shown=""
for _ in $(seq 40); do
  import -window root "$out/capture-window.png"
  if check_shot 2>/dev/null; then shown=1; break; fi
  sleep 0.5
done
[ -n "$shown" ] || { check_shot || true; fail "the capture of the focused program is not shown"; }
echo "ok: the capture of the focused program is shown"

# Press Escape in the CargoFrame window until <condition> holds (a freshly shown page may not
# be listening for keys yet, so one press can be lost).
press_escape_until() {
  local target w i
  for _ in $(seq 8); do
    target="$(find_window CargoFrame || true)"
    [ -n "$target" ] && { xdotool windowfocus "$target" 2>/dev/null || true; sleep 0.3; xdotool key --clearmodifiers Escape; }
    for i in $(seq 15); do
      if "$@"; then return 0; fi
      sleep 0.1
    done
  done
  return 1
}
not_visible() { [ -z "$(is_visible)" ]; }
no_window() { [ -z "$(find_window CargoFrame || true)" ]; }

# Memory mode defaults to Balanced: closing the window (Escape) hides it, keeps the program
# running, and the next shortcut press brings the same window back at once.
is_visible() { xdotool search --onlyvisible --name 'cargoframe' 2>/dev/null | while read -r w; do
  [ "$(xdotool getwindowname "$w" 2>/dev/null)" = "CargoFrame" ] && echo "$w"; done | head -n1; }
press_escape_until not_visible || fail "the CargoFrame window stayed visible after Escape"
kill -0 "$app_pid" 2>/dev/null || fail "CargoFrame exited when its window closed"
echo "ok: closing the window hid it and the program kept running"
xdotool windowraise "$fake_win" || true
xdotool windowfocus "$fake_win" || true
sleep 0.5
xdotool key --clearmodifiers ctrl+shift+F9
for _ in $(seq 50); do [ -n "$(is_visible)" ] && break; sleep 0.1; done
[ -n "$(is_visible)" ] || fail "the kept window did not come back on the next shortcut press"
echo "ok: the kept window came back"

# Low memory frees the web view at once: after Escape no CargoFrame window exists at all.
printf '{ "schema_version": 1, "theme": "system", "memory_mode": "low_memory", "check_updates": false }\n' > "$cfg/general.json"
press_escape_until no_window || fail "Low memory kept the window instead of destroying it"
echo "ok: Low memory destroyed the window"
xdotool windowraise "$fake_win" || true
xdotool windowfocus "$fake_win" || true
sleep 0.5
xdotool key --clearmodifiers ctrl+shift+F9
for _ in $(seq 100); do [ -n "$(find_window CargoFrame || true)" ] && break; sleep 0.1; done
[ -n "$(find_window CargoFrame || true)" ] || fail "no window after the shortcut in Low memory"
echo "ok: a new window opened in Low memory"

grep -q "panicked" "$work/cargoframe.log" && fail "the app panicked"
"$bin" --quit >/dev/null 2>&1 || true
for _ in $(seq 50); do kill -0 "$app_pid" 2>/dev/null || break; sleep 0.1; done
kill -0 "$app_pid" 2>/dev/null && fail "CargoFrame did not quit on --quit"
app_pid=""
cp "$work/cargoframe.log" "$out/cargoframe.log" 2>/dev/null || true
echo "Linux capture end-to-end check passed"
