# Installers

Planned for Phase 1. Nothing is built here yet.

## Windows

- Tauri NSIS bundler, **current-user mode**: no administrator prompt, registry entries under
  `HKCU`, default folder `%LocalAppData%\CargoFrame`.
- Start Menu shortcut with the app's AppUserModelID (needed for notification clicks).
- One installer question: "Run CargoFrame when I sign in" (default **on**). It sets a value
  under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
- Running a newer installer over an existing install: detect it, ask the running app to quit
  through the single-instance channel, unpack beside the old files, swap, keep settings, migrate
  the settings schema with a backup, keep the startup choice, ask before a downgrade.
- Uninstall asks whether to delete settings (default: keep).

## Linux

- `install.sh` wraps an AppImage: program in `~/.local/share/CargoFrame`, settings in
  `~/.config/CargoFrame`, a `.desktop` entry, and (if chosen)
  `~/.config/autostart/cargoframe.desktop`. `uninstall.sh` reverses it.
- A `.deb` follows later.
- The tray needs a StatusNotifierItem host: native on KDE, the AppIndicator extension on GNOME.
  Without one, the app opens Settings at launch with a hint.

See `docs/spikes.md` (S1, S2, S4) for what must be confirmed before this is built.
