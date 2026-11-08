#!/usr/bin/env bash
# End-to-end check on a headless Wayland desktop (sway, with D-Bus, PipeWire and the desktop
# portal), the way a person uses CargoFrame there:
#
#   1. sway: CargoFrame hands its shortcut to sway; pressing it in a Wayland program opens the
#      CargoFrame window with that program's window and its project folder.
#   2. A desktop that says nothing about focus (run as "GNOME"): the shortcut is written into
#      GNOME's keyboard settings; `cargoframe open --trigger` (what GNOME runs) captures through
#      the desktop portal and finds an XWayland program in focus.
#
#   scripts/e2e/wayland-capture.sh [path-to-cargoframe-binary] [output-dir]
#
# Needs: sway, xwayland, xdg-desktop-portal with its wlr and gtk parts, grim, wtype, foot,
# pipewire, wireplumber, python3 with Pillow, dbus-run-session; for part 2 also
# gnome-settings-daemon-common, dconf-gsettings-backend and dconf-service.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

if [ -z "${CARGOFRAME_E2E_INNER:-}" ]; then
  bin="${1:-$root/target/debug/cargoframe}"
  out="${2:-$root/target/e2e-wayland}"
  mkdir -p "$out"
  export CARGOFRAME_E2E_INNER=1 CARGOFRAME_E2E_BIN="$(readlink -f "$bin")" CARGOFRAME_E2E_OUT="$(readlink -f "$out")"
  exec dbus-run-session -- "$0"
fi

bin="$CARGOFRAME_E2E_BIN"; out="$CARGOFRAME_E2E_OUT"
fake="$root/target/debug/examples/fake_app"
[ -x "$fake" ] || { echo "build the helper first: cargo build -p cargoframe-capture --example fake_app" >&2; exit 1; }

work="$(mktemp -d)"
export XDG_RUNTIME_DIR="$work/run"; mkdir -m 700 "$XDG_RUNTIME_DIR"
export HOME="$work/home"; mkdir -p "$HOME"
# D-Bus starts services (dconf, the portals) with its own environment: give them ours.
dbus-update-activation-environment XDG_RUNTIME_DIR HOME
export XDG_CURRENT_DESKTOP=sway XDG_SESSION_TYPE=wayland XDG_SESSION_DESKTOP=sway
export WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 WLR_HEADLESS_OUTPUTS=1
unset DISPLAY WAYLAND_DISPLAY

pids=()
cleanup() {
  trap - ERR
  for p in "${app_pid:-}" "${fake_pid:-}" "${foot_pid:-}" "${pids[@]}"; do
    [ -n "$p" ] && kill "$p" 2>/dev/null || true
  done
  # the document portal mounts a folder in the runtime folder
  fusermount -u "$XDG_RUNTIME_DIR/doc" 2>/dev/null || true
  sleep 0.5
  rm -rf "$work" 2>/dev/null || true
}
trap cleanup EXIT
set -E
trap 'fail "a command failed (line $LINENO)"' ERR
fail() {
  echo "FAIL: $*" >&2
  for f in cargoframe.log cargoframe2.log sway.log xdp.log; do
    [ -f "$work/$f" ] && cp "$work/$f" "$out/$f" 2>/dev/null || true
  done
  [ -f "$work/cargoframe.log" ] && tail -n 30 "$work/cargoframe.log" >&2
  grim "$out/failure.png" 2>/dev/null || true
  exit 1
}

# ----- the desktop -----
cat > "$work/sway.cfg" <<CFG
output HEADLESS-1 resolution 1280x800 position 0,0 bg #303030 solid_color
xwayland enable
exec sh -c 'echo "\$DISPLAY" > $work/display'
CFG
sway -c "$work/sway.cfg" > "$work/sway.log" 2>&1 & pids+=($!)
for _ in $(seq 100); do ls "$XDG_RUNTIME_DIR"/wayland-? >/dev/null 2>&1 && break; sleep 0.1; done
export WAYLAND_DISPLAY="$(basename "$(ls "$XDG_RUNTIME_DIR"/wayland-? | head -n1)")"
export SWAYSOCK="$(ls "$XDG_RUNTIME_DIR"/sway-ipc.* | head -n1)"
for _ in $(seq 50); do swaymsg -t get_outputs >/dev/null 2>&1 && break; sleep 0.1; done
pipewire > "$work/pw.log" 2>&1 & pids+=($!)
sleep 0.3
wireplumber > "$work/wp.log" 2>&1 & pids+=($!)
/usr/libexec/xdg-desktop-portal-wlr > "$work/xdpw.log" 2>&1 & pids+=($!)
/usr/libexec/xdg-desktop-portal-gtk > "$work/xdpg.log" 2>&1 & pids+=($!)
/usr/libexec/xdg-desktop-portal -r > "$work/xdp.log" 2>&1 & pids+=($!)
sleep 1

export CARGOFRAME_HOME="$work/cfg" CARGOFRAME_LOG=debug CARGOFRAME_LOG_STDERR=1
cfg="$CARGOFRAME_HOME/config"
mkdir -p "$cfg/triggers" "$cfg/layouts"
cat > "$cfg/triggers/bug.json" <<'JSON'
{ "schema_version": 1, "name": "Bug report", "enabled": true, "shortcut": "Ctrl+Shift+F9",
  "capture": "active_window", "layout_id": "bug" }
JSON
cat > "$cfg/layouts/bug.json" <<'JSON'
{ "blocks": [
  { "id": "shot", "type": "screenshot", "rect": { "x": 0, "y": 0, "w": 8, "h": 9 } },
  { "id": "logs", "type": "files", "rect": { "x": 0, "y": 9, "w": 8, "h": 3 },
    "title": "Logs", "include": ["**/*.log"], "preselect": ["app.log"] },
  { "id": "send", "type": "sharing_button", "rect": { "x": 8, "y": 9, "w": 4, "h": 2 }, "label": "Share" }
] }
JSON
# The programs in focus live in <dev>/Foo/Foo, so their project is the folder Foo.
mkdir -p "$work/dev/Foo"
echo "started" > "$work/dev/Foo/app.log"

# The tree: which window is focused and where it is.
tree() { swaymsg -t get_tree; }
focused_name() { tree | python3 -c '
import json, sys
def walk(n):
    if n.get("focused") and n.get("type") in ("con", "floating_con"): print(n.get("name", "")); return True
    return any(walk(c) for c in n.get("nodes", []) + n.get("floating_nodes", []))
walk(json.load(sys.stdin))'; }
window_rect() { tree | python3 -c '
import json, sys
want = sys.argv[1]
def walk(n):
    if n.get("name") == want and n.get("type") in ("con", "floating_con"):
        r = n["rect"]; print(r["x"], r["y"], r["width"], r["height"]); return True
    return any(walk(c) for c in n.get("nodes", []) + n.get("floating_nodes", []))
walk(json.load(sys.stdin))' "$1"; }
wait_for_focus() {
  for _ in $(seq 300); do [ "$(focused_name)" = "$1" ] && return 0; sleep 0.1; done
  return 1
}
# Does the window $1 show the colour $2 (r g b) in at least 400 pixels?
shows_colour() {
  local rect; rect="$(window_rect "$1")"
  [ -n "$rect" ] || return 1
  grim "$out/$3.png"
  python3 - "$out/$3.png" $rect $2 <<'PY'
import sys
from PIL import Image
path, x, y, w, h, r, g, b = sys.argv[1], *map(int, sys.argv[2:])
im = Image.open(path).convert("RGB").crop((x, y, x + w, y + h))
n = sum(c for c, rgb in (im.getcolors(maxcolors=w * h) or []) if all(abs(a - e) <= 8 for a, e in zip(rgb, (r, g, b))))
sys.exit(0 if n >= 400 else 1)
PY
}

# ----- 1. sway: the shortcut in a Wayland program -----
cp "$(command -v foot)" "$work/dev/Foo/Foo"
"$work/dev/Foo/Foo" --title "Foo - main.rs" -o colors.background=e04f4f sleep 600 > "$work/foot.log" 2>&1 &
foot_pid=$!
wait_for_focus "Foo - main.rs" || fail "the Wayland program did not open"

"$bin" --autostart > "$work/cargoframe.log" 2>&1 &
app_pid=$!
for _ in $(seq 100); do grep -q "shortcuts go through the desktop" "$work/cargoframe.log" && break; sleep 0.1; done
grep -q "shortcuts go through the desktop r=Sway" "$work/cargoframe.log" || fail "the shortcut was not handed to sway"
echo "ok: the shortcut was handed to sway"

swaymsg "[title=\"Foo - main.rs\"] focus" >/dev/null
wait_for_focus "Foo - main.rs" || fail "the Wayland program did not take the focus back"
wtype -M ctrl -M shift -k F9 -m shift -m ctrl
wait_for_focus "CargoFrame" || fail "the CargoFrame window did not open after the shortcut"
echo "ok: the shortcut opened the CargoFrame window"
for _ in $(seq 50); do grep -q "staged=1" "$work/cargoframe.log" && break; sleep 0.1; done
grep -q "staged=1" "$work/cargoframe.log" || fail "the program's project folder was not found (no file picked)"
echo "ok: the program in focus and its project folder were found"
shown=""
for _ in $(seq 40); do
  if shows_colour "CargoFrame" "224 79 79" capture-sway; then shown=1; break; fi
  sleep 0.5
done
[ -n "$shown" ] || fail "the capture of the Wayland program is not shown"
echo "ok: the capture of the Wayland program is shown"
kill "$app_pid" "$foot_pid" 2>/dev/null || true; app_pid=""; foot_pid=""
sleep 1

# ----- 2. a desktop that does not say which window is in focus ("GNOME") -----
export DISPLAY="$(cat "$work/display" 2>/dev/null || true)"
[ -n "$DISPLAY" ] || fail "sway did not start XWayland"
rm -f "$work/dev/Foo/Foo"; cp "$fake" "$work/dev/Foo/Foo"
"$work/dev/Foo/Foo" "Foo - main.rs" > "$work/fake.out" 2>&1 &
fake_pid=$!
wait_for_focus "Foo - main.rs" || fail "the XWayland program did not open"

(
  unset SWAYSOCK
  export XDG_CURRENT_DESKTOP=GNOME
  "$bin" --autostart > "$work/cargoframe2.log" 2>&1 &
  echo $! > "$work/app2.pid"
)
app_pid="$(cat "$work/app2.pid")"
for _ in $(seq 100); do grep -q "shortcuts go through the desktop" "$work/cargoframe2.log" && break; sleep 0.1; done
if command -v gsettings >/dev/null && gsettings get org.gnome.settings-daemon.plugins.media-keys custom-keybindings >/dev/null 2>&1; then
  grep -q "r=Gnome" "$work/cargoframe2.log" || fail "GNOME's keyboard settings were not used"
  path=/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/cargoframe-bug/
  binding="$(gsettings get "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:$path" binding)"
  [ "$binding" = "'<Control><Shift>F9'" ] || fail "GNOME's shortcut is $binding"
  echo "ok: the shortcut was written into GNOME's keyboard settings"
else
  echo "SKIP  GNOME's keyboard settings (no schema here)"
fi

swaymsg "[title=\"Foo - main.rs\"] focus" >/dev/null
wait_for_focus "Foo - main.rs" || fail "the XWayland program did not take the focus back"
(unset SWAYSOCK; export XDG_CURRENT_DESKTOP=GNOME; "$bin" open --trigger bug >/dev/null 2>&1)
wait_for_focus "CargoFrame" || fail "the CargoFrame window did not open after 'cargoframe open'"
shown=""
for _ in $(seq 40); do
  # the fake program's red bar, inside the CargoFrame window: the XWayland window was found
  if shows_colour "CargoFrame" "224 79 79" capture-xwayland; then shown=1; break; fi
  sleep 0.5
done
[ -n "$shown" ] || fail "the capture of the XWayland program is not shown"
for _ in $(seq 50); do grep -q "staged=1" "$work/cargoframe2.log" && break; sleep 0.1; done
grep -q "staged=1" "$work/cargoframe2.log" || fail "the XWayland program's project folder was not found"
echo "ok: the portal took the screenshot and the XWayland program in focus was found"
echo "PASS: Wayland"
