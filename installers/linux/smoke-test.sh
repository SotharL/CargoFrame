#!/usr/bin/env bash
# Installs the real AppImage into a throw-away HOME, starts it the way sign-in would, checks it
# stays up, asks it to quit, and uninstalls. Needs xvfb, dbus-run-session.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
appimage="$(ls "$root"/target/release/bundle/appimage/*.AppImage | head -n1)"
[ -f "$appimage" ] || { echo "no AppImage built" >&2; exit 1; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
export HOME="$work/home"
mkdir -p "$HOME"
cp "$root/app/icons/128x128.png" "$work/cargoframe.png"
cp "$root/installers/linux/install.sh" "$root/installers/linux/uninstall.sh" "$work/"
cp "$appimage" "$work/"
export APPIMAGE_EXTRACT_AND_RUN=1      # CI runners have no FUSE
export WEBKIT_DISABLE_COMPOSITING_MODE=1 WEBKIT_DISABLE_DMABUF_RENDERER=1 GDK_BACKEND=x11
export CARGOFRAME_LOG=debug

prog="$HOME/.local/share/CargoFrame/CargoFrame.AppImage"
"$work/install.sh"
[ -x "$prog" ] || { echo "FAIL: not installed" >&2; exit 1; }
grep -q -- "--autostart" "$HOME/.config/autostart/CargoFrame.desktop" || { echo "FAIL: autostart entry" >&2; exit 1; }
echo "ok: installed with run on sign-in"

# Run the whole sequence inside one session bus + virtual display.
dbus-run-session -- xvfb-run -a -s "-screen 0 1280x800x24" bash -c '
  set -euo pipefail
  "'"$prog"'" --autostart >"'"$work"'/app.log" 2>&1 &
  pid=$!
  sleep 8
  if ! kill -0 "$pid" 2>/dev/null; then echo "FAIL: app exited by itself"; cat "'"$work"'/app.log"; exit 1; fi
  echo "ok: app stays running after --autostart"
  "'"$prog"'" --quit >/dev/null 2>&1 || true
  for _ in $(seq 1 20); do kill -0 "$pid" 2>/dev/null || break; sleep 0.5; done
  if kill -0 "$pid" 2>/dev/null; then echo "FAIL: app did not quit on --quit"; cat "'"$work"'/app.log"; kill "$pid"; exit 1; fi
  echo "ok: --quit stops the running copy"
'

"$HOME/.local/share/CargoFrame/uninstall.sh"
[ ! -e "$prog" ] || { echo "FAIL: program left behind" >&2; exit 1; }
echo "ok: uninstalled"
echo "AppImage smoke test passed"
