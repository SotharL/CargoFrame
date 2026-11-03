#!/usr/bin/env python3
"""Builds the CargoFrame icon set from one piece of geometry: three frames travelling right (the
"convoy"), the lead frame carrying a forward arrow.

    python3 scripts/dev/make-icons.py

Writes app/icons/source/*.svg and the PNG and ICO files the bundle, the window and the tray use.
Needs rsvg-convert (librsvg) and Pillow; uses oxipng (pip install pyoxipng) when it is present.

Two drawings, because one-pixel strokes need their own grid:
  full   64-unit grid, three frames, for 48 px and up
  small  16-unit grid, two frames, strokes centred on half pixels, for 16 to 32 px and the tray
"""
import io
import struct
import subprocess
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
ICONS = ROOT / "app" / "icons"
SOURCE = ICONS / "source"

# Brand colours. The tile is the ink of the light theme; the mark on it uses the dark theme pair.
TILE = "#101A2E"
INK_LIGHT, INK_DARK = "#101A2E", "#F1F5FB"
ACCENT_LIGHT, ACCENT_DARK = "#E8590C", "#FF7A33"


def _masks(uid: str, hide: dict[str, list[str]], area: str) -> str:
    """Knock a frame out where the frames in front of it cover it, so the mark stays
    transparent and reads on any background."""
    out = []
    for name, covers in hide.items():
        shapes = "".join(covers)
        out.append(
            f'<mask id="{uid}{name}" maskUnits="userSpaceOnUse" {area}>'
            f'<rect {area} fill="#fff"/>{shapes}</mask>'
        )
    return "<defs>" + "".join(out) + "</defs>"


def convoy_full(ink: str, accent: str, uid: str = "cf") -> str:
    """Three frames on a 64 grid (content spans x 1..61, y 15..49)."""
    area = 'x="-8" y="-8" width="80" height="80"'
    mid = '<rect x="13" y="15" width="34" height="34" rx="10"/>'
    front = '<rect x="27" y="15" width="34" height="34" rx="10"/>'
    masks = _masks(uid, {"a": [mid, front], "b": [front]}, area)
    return (
        masks
        + f'<g fill="none" style="stroke:{ink}" stroke-width="4" stroke-linejoin="round">'
        + f'<rect x="3" y="17" width="30" height="30" rx="8" stroke-opacity=".28" mask="url(#{uid}a)"/>'
        + f'<rect x="15" y="17" width="30" height="30" rx="8" stroke-opacity=".6" mask="url(#{uid}b)"/>'
        + '<rect x="29" y="17" width="30" height="30" rx="8"/></g>'
        + f'<path d="M41 26l7 6-7 6" fill="none" style="stroke:{accent}" stroke-width="4" '
        + 'stroke-linecap="round" stroke-linejoin="round"/>'
    )


def convoy_small(ink: str, accent: str, uid: str = "cf") -> str:
    """Two frames on a 16 grid; every stroke is one unit wide and centred on a half unit, so it
    lands on whole pixels at 16 px and at 32 px."""
    area = 'x="-2" y="-2" width="20" height="20"'
    front = '<rect x="6" y="4" width="8" height="8" rx="2"/>'
    masks = _masks(uid, {"a": [front]}, area)
    return (
        masks
        + f'<g fill="none" style="stroke:{ink}" stroke-width="1" stroke-linejoin="round">'
        + f'<rect x="2.5" y="4.5" width="7" height="7" rx="1.5" stroke-opacity=".6" mask="url(#{uid}a)"/>'
        + '<rect x="6.5" y="4.5" width="7" height="7" rx="1.5"/></g>'
        + f'<path d="M9.25 6.5L11 8 9.25 9.5" fill="none" style="stroke:{accent}" stroke-width="1" '
        + 'stroke-linecap="round" stroke-linejoin="round"/>'
    )


def svg(view: int, body: str, extra: str = "") -> str:
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {view} {view}"{extra}>'
        f"{body}</svg>\n"
    )


def tile_full() -> str:
    body = (
        f'<rect width="64" height="64" rx="14" fill="{TILE}"/>'
        '<g transform="translate(32 32) scale(.78) translate(-31 -32)">'
        + convoy_full(INK_DARK, ACCENT_DARK)
        + "</g>"
    )
    return svg(64, body)


def tile_small() -> str:
    body = f'<rect width="16" height="16" rx="3.5" fill="{TILE}"/>' + convoy_small(INK_DARK, ACCENT_DARK)
    return svg(16, body)


def mark_adaptive() -> str:
    """The mark alone, transparent, following the page's light or dark theme."""
    style = (
        "<style>:root{--ink:%s;--accent:%s}"
        "@media (prefers-color-scheme:dark){:root{--ink:%s;--accent:%s}}</style>"
        % (INK_LIGHT, ACCENT_LIGHT, INK_DARK, ACCENT_DARK)
    )
    body = style + convoy_full("var(--ink)", "var(--accent)")
    return svg(64, body, ' role="img" aria-label="CargoFrame"')


def render(svg_text: str, size: int) -> bytes:
    out = subprocess.run(
        ["rsvg-convert", "-w", str(size), "-h", str(size), "-f", "png"],
        input=svg_text.encode(),
        capture_output=True,
        check=True,
    ).stdout
    return optimize(out)


def optimize(png: bytes) -> bytes:
    """Lossless: same RGBA pixels, smaller file. The colour type stays 8-bit RGBA, because
    Tauri's icon and tray loaders do not take indexed PNGs."""
    try:
        import oxipng

        return oxipng.optimize_from_memory(
            png,
            level=4,
            color_type_reduction=False,
            palette_reduction=False,
            grayscale_reduction=False,
            bit_depth_reduction=False,
        )
    except ImportError:
        buf = io.BytesIO()
        Image.open(io.BytesIO(png)).convert("RGBA").save(buf, "PNG", optimize=True)
        return buf.getvalue()


def build_ico(entries: list[tuple[int, bytes]]) -> bytes:
    head = struct.pack("<HHH", 0, 1, len(entries))
    offset = 6 + 16 * len(entries)
    table, data = b"", b""
    for size, png in entries:
        w = 0 if size >= 256 else size
        table += struct.pack("<BBBBHHII", w, w, 0, 0, 1, 32, len(png), offset)
        data += png
        offset += len(png)
    return head + table + data


def main() -> int:
    SOURCE.mkdir(parents=True, exist_ok=True)
    full, small = tile_full(), tile_small()
    (SOURCE / "icon.svg").write_text(full)
    (SOURCE / "icon-small.svg").write_text(small)
    (SOURCE / "mark.svg").write_text(mark_adaptive())

    # Bundle and window icons. 32x32.png is also the tray icon and the default window icon.
    outputs = {
        "icon.png": render(full, 512),
        "128x128@2x.png": render(full, 256),
        "128x128.png": render(full, 128),
        "32x32.png": render(small, 32),
    }
    for name, data in outputs.items():
        (ICONS / name).write_bytes(data)

    ico = [(s, render(small, s)) for s in (16, 24, 32)] + [(s, render(full, s)) for s in (48, 64, 128, 256)]
    (ICONS / "icon.ico").write_bytes(build_ico(ico))

    for name in [*outputs, "icon.ico"]:
        print(f"{name}: {(ICONS / name).stat().st_size} bytes")
    return 0


if __name__ == "__main__":
    sys.exit(main())
