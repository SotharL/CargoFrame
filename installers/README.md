# Installers

Both installers are per-user: no administrator or root prompt, nothing outside the user's own
folders. Both are built and smoke-tested by `.github/workflows/bundle.yml` on every push, and
built, checked and drafted as a release by `.github/workflows/release.yml` for a version tag, or by
hand from the Actions tab (**Release, Run workflow**) with the tag left empty: the draft then points at
the commit that was built and publishing it creates the tag `v<version>`, so no tag has to be pushed.

## Windows (`installers/windows/`)

- Build it with `scripts\windows\build.cmd` (see the README), or in VS Code with Ctrl+Shift+B.
  CI builds it with the same script. The first build downloads the NSIS tools once.
- Tauri's NSIS bundler in **current-user mode**; `installer.nsi` and `hooks.nsh` hold our
  changes. Registry entries are under `HKCU`; the default folder is `%LocalAppData%\CargoFrame`.
- Start Menu shortcut with the app's AppUserModelID (needed for notification clicks).
- **One page, one button.** The window says what will happen and has a single button (**Install**,
  or **Update** over an existing copy). No welcome, folder, shortcut or options pages and no finish
  page: the progress bar closes by itself and the installer opens CargoFrame, which lands on its
  Setup page the first time. "Run CargoFrame when I sign in" is **on** (stored as a value under
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`; an upgrade keeps a person's earlier choice;
  Settings, General changes it; `/NOSTARTUP` leaves it off for a silent install).
- Nothing else is needed on the PC to install: the web view (WebView2) ships with Windows 11 and is
  fetched from Microsoft by the installer only if it is missing. ffmpeg is separate and optional;
  the app's Setup page installs it (see `docs/decisions.md`).
- Running a newer installer over an existing install quits the running app through its
  single-instance channel (not a kill), swaps the files, keeps settings and the startup choice.
- Uninstall asks whether to delete settings (default: keep).
- `smoke-test.ps1` installs silently, starts the app with `--autostart`, measures its idle cost,
  upgrades over it, uninstalls, and checks what stayed behind. `click-test.ps1` then does what a
  person does: opens the installer window, checks the button says Install, presses it, waits for the
  window to close by itself and checks the app was installed and opened a window (screenshots of
  each step are kept as the `installer-screenshots` artifact). CI runs both on a Windows runner.
- The release (`release.yml`, for a version tag) names the Windows file `CargoFrame-Setup.exe`, so
  `.../releases/latest/download/CargoFrame-Setup.exe` is always the one download.
- **Signing:** the release workflow signs the installer when the certificate secrets exist
  (`scripts/windows/sign.ps1`). The program inside the installer is **not** signed yet. See
  `docs/signing.md`.

## Linux (`installers/linux/`)

- `install.sh` wraps an AppImage: program in `~/.local/share/CargoFrame`, settings in
  `~/.config/CargoFrame`, a `.desktop` entry, and (if chosen) `~/.config/autostart/CargoFrame.desktop`.
  `uninstall.sh` reverses it and keeps settings unless you pass `--delete-data` (which also
  removes kept zips and logs). Install options: `--no-startup`, `--startup`, `--appimage FILE`.
- `smoke-test.sh` installs and runs the real AppImage under a virtual display and quits it.
  `test-install.sh` checks the scripts against a fake AppImage in a throw-away home folder
  (upgrade keeps settings, the startup choice is kept). `session-check.sh` is what the smoke test
  runs inside the virtual session: start the way sign-in would, stay up, quit when asked.
- x64 only. A `.deb` is a later option.
- The tray needs a StatusNotifierItem host: native on KDE, the AppIndicator extension on GNOME.
  Without one, the app opens Settings at launch with a hint.

## Not covered by automated tests

A real Windows 11 install by a standard user (spike S1), notification clicks (S2), and the tray
on real GNOME and KDE sessions (S4). See `docs/spikes.md`.
