// A list that opens under (or over) a box, fixed to the screen rather than to the block that
// holds the box: a block clips what overflows it (the CargoFrame window's cells scroll), and a
// list must reach past a small block without pushing the blocks under it. Used by the dropdown's
// combobox and the Tags block.

export type Place = { left: number; top: number; width: number; maxH: number; up: boolean };

/** Where a list goes for a box at `r`, in a window `w` × `h`. Pure, so it is tested. */
export function placeUnder(r: DOMRect | { left: number; top: number; bottom: number; width: number }, w: number, h: number, minWidth = 260, maxH = 240): Place {
  const below = h - r.bottom - 8;
  const above = r.top - 8;
  const up = below < 160 && above > below;
  // a narrow box (one of three in a row) still gets a list wide enough to read, kept on screen
  const width = Math.min(Math.max(r.width, minWidth), w - 16);
  return {
    left: Math.max(8, Math.min(r.left, w - width - 8)),
    width,
    top: up ? r.top - 2 : r.bottom + 2,
    maxH: Math.max(100, Math.min(maxH, up ? above : below)),
    up,
  };
}

/**
 * Svelte action: keeps `node` (a list) fixed under `anchor`, following it while anything scrolls
 * or the window is resized.
 */
export function floatUnder(node: HTMLElement, anchor: HTMLElement | undefined) {
  let at = anchor;
  const put = () => {
    if (!at) return;
    const p = placeUnder(at.getBoundingClientRect(), window.innerWidth, window.innerHeight);
    Object.assign(node.style, {
      position: 'fixed',
      zIndex: '50',
      left: `${p.left}px`,
      top: `${p.top}px`,
      width: `${p.width}px`,
      maxHeight: `${p.maxH}px`,
      transform: p.up ? 'translateY(-100%)' : '',
    });
  };
  put();
  window.addEventListener('resize', put);
  window.addEventListener('scroll', put, true);
  return {
    update(next: HTMLElement | undefined) {
      at = next;
      put();
    },
    destroy() {
      window.removeEventListener('resize', put);
      window.removeEventListener('scroll', put, true);
    },
  };
}
