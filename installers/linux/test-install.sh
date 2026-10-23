#!/usr/bin/env bash
# Exercises install.sh and uninstall.sh against a fake AppImage in a throw-away HOME.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
work="$(mktemp -d "${TMPDIR:-/tmp}/cf install test.XXXXXX")"   # a space on purpose
trap 'rm -rf "$work"' EXIT

fail() { echo "FAIL: $*" >&2; exit 1; }
ok() { echo "ok: $*"; }

export HOME="$work/home dir"
unset XDG_DATA_HOME XDG_CONFIG_HOME XDG_CURRENT_DESKTOP
mkdir -p "$HOME"
prog="$HOME/.local/share/CargoFrame"
cfg="$HOME/.config/CargoFrame"
auto="$HOME/.config/autostart/CargoFrame.desktop"

make_fake() { # version file
  cat > "$2" <<FAKE
#!/bin/sh
# fake CargoFrame $1
echo "\$@" >> "$work/calls.log"
exit 0
FAKE
  chmod +x "$2"
}

mkdir -p "$work/rel1" "$work/rel2"
cp "$here/install.sh" "$here/uninstall.sh" "$work/rel1/"
cp "$here/install.sh" "$here/uninstall.sh" "$work/rel2/"
make_fake 1 "$work/rel1/CargoFrame_1.0.0_amd64.AppImage"
make_fake 2 "$work/rel2/CargoFrame_2.0.0_amd64.AppImage"
printf 'png' > "$work/rel1/cargoframe.png"

# 1. fresh install
"$work/rel1/install.sh" >/dev/null
[ -x "$prog/CargoFrame.AppImage" ] || fail "program missing"
grep -q "fake CargoFrame 1" "$prog/CargoFrame.AppImage" || fail "wrong program"
[ -x "$prog/uninstall.sh" ] || fail "uninstall.sh not copied"
[ -f "$HOME/.local/share/icons/hicolor/128x128/apps/cargoframe.png" ] || fail "icon missing"
grep -q "^Exec=\"$prog/CargoFrame.AppImage\"$" "$HOME/.local/share/applications/cargoframe.desktop" || fail "menu entry Exec"
grep -q "^Exec=\"$prog/CargoFrame.AppImage\" --autostart$" "$auto" || fail "autostart entry Exec"
[ ! -e "$work/calls.log" ] || fail "fresh install must not start or quit anything"
ok "fresh install, run on sign-in on by default"

# 2. upgrade keeps settings and startup, asks the old copy to quit
mkdir -p "$cfg" "$prog/data"
echo '{"schema_version":1}' > "$cfg/general.json"
echo log > "$prog/data/app.log"
"$work/rel2/install.sh" >/dev/null
grep -q "fake CargoFrame 2" "$prog/CargoFrame.AppImage" || fail "not upgraded"
grep -qx -- "--quit" "$work/calls.log" || fail "old copy was not asked to quit"
[ -f "$cfg/general.json" ] && [ -f "$prog/data/app.log" ] || fail "settings or data lost on upgrade"
[ -f "$auto" ] || fail "startup lost on upgrade"
ls -A "$prog" | grep -q '^\.CargoFrame' && fail "temp file left behind"
ok "upgrade in place keeps settings, data and startup"

# 3. an upgrade respects a person who turned run-on-sign-in off
rm -f "$auto"
"$work/rel2/install.sh" >/dev/null
[ ! -e "$auto" ] || fail "startup came back after being turned off"
"$work/rel2/install.sh" --startup >/dev/null
[ -f "$auto" ] || fail "--startup ignored"
ok "startup choice preserved, --startup forces it on"

# 4. uninstall keeps settings by default
"$prog/uninstall.sh" >/dev/null
[ ! -e "$prog/CargoFrame.AppImage" ] || fail "program not removed"
[ ! -e "$auto" ] && [ ! -e "$HOME/.local/share/applications/cargoframe.desktop" ] || fail "entries not removed"
[ -f "$cfg/general.json" ] && [ -f "$prog/data/app.log" ] || fail "uninstall deleted data without being asked"
ok "uninstall keeps settings and data"

# 5. fresh --no-startup, then uninstall --delete-data
"$work/rel1/install.sh" --no-startup >/dev/null
[ ! -e "$auto" ] || fail "--no-startup ignored"
"$prog/uninstall.sh" --delete-data >/dev/null
[ ! -e "$cfg" ] && [ ! -e "$prog" ] || fail "--delete-data left files behind"
ok "--no-startup and --delete-data"

# 6. bad input
if "$work/rel1/install.sh" --appimage /nonexistent >/dev/null 2>&1; then fail "missing AppImage accepted"; fi
if "$work/rel1/install.sh" --bogus >/dev/null 2>&1; then fail "unknown option accepted"; fi
ok "bad input refused"
echo "all install tests passed"
