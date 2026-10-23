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
import -window root "$out/capture-window.png"
python3 - "$out/capture-window.png" <<'PY'
import sys
from PIL import Image
im = Image.open(sys.argv[1]).convert("RGB")
w, h = im.size
colors = im.getcolors(maxcolors=w * h) or []
print(f"screen {w}x{h}, {len(colors)} distinct colours")
def has(rgb, tol=6, minimum=200):
    return sum(n for n, c in colors if all(abs(a - b) <= tol for a, b in zip(c, rgb))) >= minimum
# The captured program's red strip and blue bar must be visible inside the window.
assert has((0xE0, 0x4F, 0x4F)), "the program's red strip is not on screen (capture not shown?)"
assert has((0x4F, 0x8E, 0xE0)), "the program's blue bar is not on screen"
print("ok: the capture of the focused program is shown")
PY

grep -q "panicked" "$work/cargoframe.log" && fail "the app panicked"
"$bin" --quit >/dev/null 2>&1 || true
for _ in $(seq 50); do kill -0 "$app_pid" 2>/dev/null || break; sleep 0.1; done
kill -0 "$app_pid" 2>/dev/null && fail "CargoFrame did not quit on --quit"
app_pid=""
cp "$work/cargoframe.log" "$out/cargoframe.log" 2>/dev/null || true
echo "Linux capture end-to-end check passed"
