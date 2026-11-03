# CargoFrame brand

The mark is a **convoy**: three frames moving right, the lead one carrying a forward arrow. The frames are the screenshots, the convoy is the bulk, the arrow is the sharing.

## Colours

| Role | Light surfaces | Dark surfaces |
|---|---|---|
| Frames (ink) | `#101A2E` | `#F1F5FB` |
| Arrow (accent) | `#E8590C` | `#FF7A33` |
| App icon tile | `#101A2E` | `#101A2E` |

The app icon always uses the tile with the dark-surface pair on it, so it reads on any taskbar, dock or file list. Inside the app the mark has no tile: the frames take the theme's text colour (`--text`) and the arrow takes `--brand-accent` (see `ui/src/lib/tokens.css`).

## Which file for which surface

| Surface | File |
|---|---|
| Windows executable, installer, Add/Remove Programs | `app/icons/icon.ico` (16, 24, 32 use the small drawing; 48 to 256 the full one) |
| Tray icon and default window icon | `app/icons/32x32.png` (small drawing) |
| Linux menu and notifications | `app/icons/128x128.png`, installed as `cargoframe.png` by `installers/linux/install.sh` |
| Tauri bundle, high-DPI | `app/icons/128x128@2x.png`, `app/icons/icon.png` |
| Settings sidebar | `ui/src/lib/components/Logo.svelte` next to the name |
| Anywhere else a transparent mark is needed (docs, web) | `app/icons/source/mark.svg` (follows the page's light or dark theme) |

The name is set as **Cargo** in a heavy weight and **Frame** in a medium weight, one word, no space, in the system font.

## Two drawings

One-pixel strokes need their own grid, so there are two drawings. The **full** one (64-unit grid, three frames) is for 48 px and up. The **small** one (16-unit grid, two frames, strokes on half units) lands on whole pixels at 16 px and 32 px, and is used for the tray. Do not scale the full drawing below 48 px.

## Regenerating

Everything under `app/icons/` is generated:

```
python3 scripts/dev/make-icons.py
```

It needs `rsvg-convert` (librsvg) and Pillow, and uses `oxipng` (`pip install pyoxipng`) when present. The PNGs stay 8-bit RGBA on purpose: Tauri's icon and tray loaders do not take indexed PNGs. If the geometry changes, change it in the script and in `Logo.svelte`, which holds the same shapes.
