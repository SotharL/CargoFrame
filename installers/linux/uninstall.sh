#!/usr/bin/env bash
# Remove CargoFrame (per user).
#
#   ./uninstall.sh                keep your settings and local data
#   ./uninstall.sh --delete-data  also delete settings, zips kept after failures, and logs
set -euo pipefail

data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
config_home="${XDG_CONFIG_HOME:-$HOME/.config}"
prog_dir="$data_home/CargoFrame"
target="$prog_dir/CargoFrame.AppImage"

delete_data=no
for arg in "$@"; do
  case "$arg" in
    --delete-data) delete_data=yes ;;
    -h|--help) sed -n '2,6p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "uninstall.sh: unknown option: $arg" >&2; exit 1 ;;
  esac
done

if [ -e "$target" ]; then
  APPIMAGE_EXTRACT_AND_RUN=1 timeout 10 "$target" --quit >/dev/null 2>&1 || true
  sleep 1
fi

# On GNOME (Wayland), CargoFrame's shortcuts live in GNOME's keyboard settings: take them out.
# Only the ones it made (their path has "cargoframe-" in it) are touched.
schema=org.gnome.settings-daemon.plugins.media-keys
if command -v gsettings >/dev/null 2>&1 && gsettings get "$schema" custom-keybindings >/dev/null 2>&1; then
  list="$(gsettings get "$schema" custom-keybindings)"
  for path in $(printf '%s' "$list" | grep -o "'[^']*/cargoframe-[^']*'" | tr -d "'"); do
    gsettings reset-recursively "$schema.custom-keybinding:$path" 2>/dev/null || true
  done
  kept="$(printf '%s' "$list" | sed -E "s#'[^']*/cargoframe-[^']*'(, )?##g; s#, \]#]#")"
  [ "$kept" = "[]" ] && kept="@as []"
  [ "$kept" != "$list" ] && gsettings set "$schema" custom-keybindings "$kept" || true
fi

rm -f "$target" "$prog_dir/uninstall.sh"
rm -f "$data_home/applications/cargoframe.desktop"
rm -f "$config_home/autostart/CargoFrame.desktop"
rm -f "$data_home/icons/hicolor/128x128/apps/cargoframe.png"

if [ "$delete_data" = yes ]; then
  rm -rf "$config_home/CargoFrame" "$prog_dir/data"
fi
# Remove the program folder only when nothing of the person's is left in it.
rmdir "$prog_dir" 2>/dev/null || true

command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$data_home/applications" >/dev/null 2>&1 || true

if [ "$delete_data" = yes ]; then echo "CargoFrame removed, with its settings and data."; else echo "CargoFrame removed. Settings kept in $config_home/CargoFrame."; fi
