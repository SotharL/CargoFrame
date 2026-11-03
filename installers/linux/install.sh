#!/usr/bin/env bash
# CargoFrame installer for Linux (per user, no root).
#
#   ./install.sh                      install or upgrade in place, run on sign-in ON for a fresh install
#   ./install.sh --no-startup         do not run on sign-in
#   ./install.sh --startup            run on sign-in, even on an upgrade that had it off
#   ./install.sh --appimage FILE      use this AppImage (default: the CargoFrame*.AppImage next to this script)
#   ./install.sh --restart            start CargoFrame again when done (used by the in-app update)
#
# Program:   ${XDG_DATA_HOME:-~/.local/share}/CargoFrame/CargoFrame.AppImage
# Settings:  ${XDG_CONFIG_HOME:-~/.config}/CargoFrame            (never touched here)
# Entries:   applications/cargoframe.desktop, autostart/CargoFrame.desktop, icons/.../cargoframe.png
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
config_home="${XDG_CONFIG_HOME:-$HOME/.config}"
prog_dir="$data_home/CargoFrame"
target="$prog_dir/CargoFrame.AppImage"
apps_dir="$data_home/applications"
icon_dir="$data_home/icons/hicolor/128x128/apps"
autostart_file="$config_home/autostart/CargoFrame.desktop"

startup=default
appimage=""
restart=no

die() { echo "install.sh: $*" >&2; exit 1; }

while [ $# -gt 0 ]; do
  case "$1" in
    --no-startup) startup=off ;;
    --startup) startup=on ;;
    --restart) restart=yes ;;
    --appimage) shift; [ $# -gt 0 ] || die "--appimage needs a file"; appimage="$1" ;;
    -h|--help) sed -n '2,13p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) die "unknown option: $1" ;;
  esac
  shift
done

if [ -z "$appimage" ]; then
  shopt -s nullglob
  found=("$here"/CargoFrame*.AppImage)
  shopt -u nullglob
  [ ${#found[@]} -eq 1 ] || die "put exactly one CargoFrame*.AppImage next to this script, or pass --appimage FILE"
  appimage="${found[0]}"
fi
[ -f "$appimage" ] || die "not a file: $appimage"

upgrade=no
[ -e "$target" ] && upgrade=yes

# 1. Ask a running copy to quit (it forwards through its single-instance channel and exits).
if [ "$upgrade" = yes ]; then
  APPIMAGE_EXTRACT_AND_RUN=1 timeout 10 "$target" --quit >/dev/null 2>&1 || true
  sleep 1
fi

# 2. Put the program in place. Write beside the old file, then rename, so a failure never
#    leaves a half-written program.
mkdir -p "$prog_dir"
tmp="$(mktemp "$prog_dir/.CargoFrame.AppImage.XXXXXX")"
trap 'rm -f "$tmp"' EXIT
cp "$appimage" "$tmp"
chmod 755 "$tmp"
mv -f "$tmp" "$target"
trap - EXIT

# The uninstall script travels with the program.
if [ -f "$here/uninstall.sh" ]; then
  install -m 755 "$here/uninstall.sh" "$prog_dir/uninstall.sh"
fi

# 3. Icon and menu entry.
if [ -f "$here/cargoframe.png" ]; then
  mkdir -p "$icon_dir"
  install -m 644 "$here/cargoframe.png" "$icon_dir/cargoframe.png"
fi
mkdir -p "$apps_dir"
cat > "$apps_dir/cargoframe.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Version=1.0
Name=CargoFrame
Comment=Capture, review and share your screen with its context
Exec="$target"
Icon=cargoframe
Terminal=false
Categories=Utility;
StartupNotify=false
DESKTOP

# 4. Run on sign-in. Fresh install: on. Upgrade: keep what the person had.
want_startup=no
case "$startup" in
  on) want_startup=yes ;;
  off) want_startup=no ;;
  default)
    if [ "$upgrade" = yes ]; then
      [ -f "$autostart_file" ] && want_startup=yes
    else
      want_startup=yes
    fi
    ;;
esac
if [ "$want_startup" = yes ]; then
  mkdir -p "$(dirname "$autostart_file")"
  cat > "$autostart_file" <<DESKTOP
[Desktop Entry]
Type=Application
Version=1.0
Name=CargoFrame
Comment=CargoFrame startup script
Exec="$target" --autostart
StartupNotify=false
Terminal=false
DESKTOP
else
  rm -f "$autostart_file"
fi

command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$apps_dir" >/dev/null 2>&1 || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q -t "$data_home/icons/hicolor" >/dev/null 2>&1 || true

# 5. Tell the person about the one thing that can stop the tray icon from showing.
if [ "${XDG_CURRENT_DESKTOP:-}" != "" ] && echo "${XDG_CURRENT_DESKTOP}" | grep -qi gnome; then
  if command -v gnome-extensions >/dev/null 2>&1 && ! gnome-extensions list 2>/dev/null | grep -qi appindicator; then
    echo "Note: GNOME does not show tray icons by itself. Install the 'AppIndicator and KStatusNotifierItem Support' extension, or CargoFrame will open Settings when it starts."
  fi
fi

if [ "$upgrade" = yes ]; then echo "CargoFrame upgraded: $target"; else echo "CargoFrame installed: $target"; fi
if [ "$want_startup" = yes ]; then echo "It will run when you sign in."; else echo "It will not run when you sign in."; fi
echo "Start it from your applications menu, or run: $target"

# 6. Started again by the in-app update: quit above, so it is safe to start the new copy now.
if [ "$restart" = yes ]; then
  if command -v setsid >/dev/null 2>&1; then
    setsid -f "$target" >/dev/null 2>&1 </dev/null || true
  else
    nohup "$target" >/dev/null 2>&1 </dev/null &
  fi
  echo "CargoFrame was started again."
fi
