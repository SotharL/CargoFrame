/**
 * The layout builder's rules, kept apart from the screen so they can be tested on their own. They
 * mirror core/layout/grid.rs: a 12-column grid, whole cells, blocks never overlap.
 */
import type { Block, BlockContent, Layout, Rect } from '../../lib/types';

export const COLUMNS = 12;
/** The canvas always shows at least this many rows, plus a spare row to drop into. */
export const MIN_ROWS = 10;

/** A window has room for this many sharing buttons (the same limit as core/layout/validate.rs). */
export const MAX_SHARING_BUTTONS = 5;

export type BlockType = BlockContent['type'];

export const BLOCK_TYPES: { type: BlockType; label: string; hint: string; size: [number, number] }[] = [
  { type: 'screenshot', label: 'Screenshot', hint: 'The capture, with crop, draw, shapes and text', size: [8, 8] },
  { type: 'video', label: 'Video', hint: 'The recording of a video trigger, to play, trim, crop and mark', size: [8, 8] },
  { type: 'text', label: 'Text', hint: 'A text box with a title, for notes or a prompt', size: [4, 3] },
  { type: 'dropdown', label: 'Dropdown', hint: 'Choices from a service or from your Claude chats', size: [4, 2] },
  { type: 'files', label: 'Files', hint: 'Files from the project folder (or folders above the program) to tick and zip together', size: [6, 3] },
  { type: 'sharing_button', label: 'Sharing button', hint: 'Sends everything to a connection', size: [4, 2] },
];

export const typeLabel = (t: BlockType) => BLOCK_TYPES.find((b) => b.type === t)?.label ?? t;

export function overlaps(a: Rect, b: Rect): boolean {
  return a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
}

export function fits(r: Rect): boolean {
  return r.w >= 1 && r.h >= 1 && r.x >= 0 && r.y >= 0 && r.x + r.w <= COLUMNS;
}

export function canPlace(blocks: readonly Block[], rect: Rect, ignoreId?: string): boolean {
  return fits(rect) && !blocks.some((b) => b.id !== ignoreId && overlaps(b.rect, rect));
}

/** Rows to draw: everything placed, plus a spare row, never fewer than [`MIN_ROWS`]. */
export function rowsNeeded(blocks: readonly Block[]): number {
  const bottom = blocks.reduce((m, b) => Math.max(m, b.rect.y + b.rect.h), 0);
  return Math.max(MIN_ROWS, bottom + 2);
}

/** The first spot (reading order) where a `w` by `h` block fits. Always exists: rows are unbounded. */
export function firstFree(blocks: readonly Block[], w: number, h: number): Rect {
  const width = Math.min(Math.max(1, w), COLUMNS);
  for (let y = 0; ; y++) {
    for (let x = 0; x + width <= COLUMNS; x++) {
      const r = { x, y, w: width, h };
      if (canPlace(blocks, r)) return r;
    }
  }
}

const ID_PREFIX: Record<BlockType, string> = {
  screenshot: 'shot',
  video: 'clip',
  text: 'text',
  dropdown: 'dropdown',
  files: 'files',
  sharing_button: 'share',
};

/** An id not used by any block, like `text-2`. */
export function newBlockId(blocks: readonly Block[], type: BlockType): string {
  const used = new Set(blocks.map((b) => b.id));
  const prefix = ID_PREFIX[type];
  if ((type === 'screenshot' || type === 'video') && !used.has(prefix)) return prefix;
  for (let n = 1; ; n++) {
    const id = `${prefix}-${n}`;
    if (!used.has(id)) return id;
  }
}

function content(type: BlockType): BlockContent {
  switch (type) {
    case 'screenshot':
    case 'video':
      return { type };
    case 'text':
      return { type, title: 'Notes', placement: 'above', role: 'plain', default_text: '' };
    case 'dropdown':
      return { type, title: 'Choose', source: { type: 'claude_chats' } };
    case 'files':
      return {
        type, title: 'Files', include: ['**/*.log'], preselect: [], look_in: { folders: ['project'], subfolder: '' },
      };
    case 'sharing_button':
      return {
        type, label: 'Share', connection_id: null, prompt_block_id: null, mappings: [], on_success: 'close_window',
      };
  }
}

/** A new block of `type` placed at `at` (or the first free spot), with a fresh id. */
export function makeBlock(blocks: readonly Block[], type: BlockType, at?: { x: number; y: number }): Block | null {
  if (type === 'sharing_button' && blocks.filter((b) => b.type === 'sharing_button').length >= MAX_SHARING_BUTTONS) return null;
  const [w, h] = BLOCK_TYPES.find((b) => b.type === type)!.size;
  let rect: Rect;
  if (at) {
    const x = Math.min(Math.max(0, at.x), COLUMNS - w);
    rect = { x, y: Math.max(0, at.y), w, h };
    if (!canPlace(blocks, rect)) return null;
  } else {
    rect = firstFree(blocks, w, h);
  }
  return { id: newBlockId(blocks, type), rect, ...content(type) } as Block;
}

function replaceRect(blocks: readonly Block[], id: string, rect: Rect): Block[] | null {
  if (!canPlace(blocks, rect, id)) return null;
  return blocks.map((b) => (b.id === id ? { ...b, rect } : b));
}

/** Move to a cell, keeping the block inside the grid. `null` when something is in the way. */
export function moveTo(blocks: readonly Block[], id: string, x: number, y: number): Block[] | null {
  const b = blocks.find((k) => k.id === id);
  if (!b) return null;
  const nx = Math.min(Math.max(0, x), COLUMNS - b.rect.w);
  return replaceRect(blocks, id, { ...b.rect, x: nx, y: Math.max(0, y) });
}

/** Resize, at least one cell, within the grid. `null` when something is in the way. */
export function resizeTo(blocks: readonly Block[], id: string, w: number, h: number): Block[] | null {
  const b = blocks.find((k) => k.id === id);
  if (!b) return null;
  const nw = Math.min(Math.max(1, w), COLUMNS - b.rect.x);
  return replaceRect(blocks, id, { ...b.rect, w: nw, h: Math.max(1, h) });
}

/**
 * Remove a block and everything that pointed at it: a sharing button's prompt block and any
 * mapping that took its value from the block. Stale references would only show up later as
 * confusing warnings.
 */
export function removeBlock(blocks: readonly Block[], id: string): Block[] {
  return blocks
    .filter((b) => b.id !== id)
    .map((b) => {
      if (b.type !== 'sharing_button') return b;
      return {
        ...b,
        prompt_block_id: b.prompt_block_id === id ? null : b.prompt_block_id,
        mappings: b.mappings.filter((m) => !(m.source.kind === 'block' && m.source.value === id)),
      };
    });
}

/** Where the pointer is, as a grid cell, given the grid's pixel size and the row height. */
export function cellAt(
  px: number, py: number, gridWidth: number, rowHeight: number, gap: number,
): { x: number; y: number } {
  const cellW = (gridWidth - gap * (COLUMNS - 1)) / COLUMNS;
  return {
    x: Math.floor(px / (cellW + gap)),
    y: Math.floor(py / (rowHeight + gap)),
  };
}

/** A copy of the layout with its blocks in reading order, so saved files diff cleanly. */
export function tidy(layout: Layout): Layout {
  const blocks = [...layout.blocks].sort((a, b) => a.rect.y - b.rect.y || a.rect.x - b.rect.x);
  return { ...layout, blocks };
}
