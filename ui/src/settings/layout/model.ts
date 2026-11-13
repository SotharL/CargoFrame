/**
 * The layout builder's rules, kept apart from the screen so they can be tested on their own. They
 * mirror core/layout/grid.rs: a grid of 12 columns (or more, up to 24, for a wider window), whole
 * cells, blocks never overlap, except in a Tabs block: a block dropped inside one goes in its
 * open tab, and blocks of different tabs take the same cells (lib/tabs.ts).
 */
import type { Block, BlockContent, Layout, Rect } from '../../lib/types';
import { MIN_TABS_SIZE, childIds, openTab, tabOf, type OpenTabs, type TabsBlock } from '../../lib/tabs';

/** The columns of a layout that does not say, and the fewest and most a layout can have. */
export const COLUMNS = 12;
export const MIN_COLUMNS = 12;
export const MAX_COLUMNS = 24;

/** A layout's columns, within what a layout can have. */
export const columnsOf = (l: Pick<Layout, 'columns'>): number =>
  Math.min(MAX_COLUMNS, Math.max(MIN_COLUMNS, Math.round(l.columns ?? COLUMNS)));

/** The columns the blocks reach: the grid cannot be narrower. */
export const usedColumns = (blocks: readonly Block[]): number => blocks.reduce((m, b) => Math.max(m, b.rect.x + b.rect.w), 0);
/** The canvas always shows at least this many rows, plus a spare row to drop into. */
export const MIN_ROWS = 10;

/** A window has room for this many sharing buttons (the same limit as core/layout/validate.rs). */
export const MAX_SHARING_BUTTONS = 5;

export type BlockType = BlockContent['type'];

export const BLOCK_TYPES: { type: BlockType; label: string; hint: string; size: [number, number] }[] = [
  { type: 'screenshot', label: 'Screenshot', hint: 'The capture, with crop, draw, shapes and text', size: [8, 8] },
  { type: 'video', label: 'Video', hint: 'A recording of the screen: a shortcut that opens this layout records, and the window plays, trims, crops and marks it', size: [8, 8] },
  { type: 'text', label: 'Text', hint: 'A text box with a title, for notes or a prompt', size: [4, 3] },
  { type: 'dropdown', label: 'Dropdown', hint: 'Choices from a service or from your Claude chats', size: [4, 2] },
  { type: 'files', label: 'Files', hint: 'Files from the project folder (or folders above the program) to tick and zip together', size: [6, 3] },
  { type: 'tags', label: 'Tags', hint: 'Tags like Jira labels: started from the capture’s values, up to 25, sent in a field', size: [4, 1] },
  { type: 'log_view', label: 'Log viewer', hint: 'Shows a file of the Files block line by line, with a filter, to copy lines from', size: [6, 4] },
  { type: 'tabs', label: 'Tabs', hint: 'Tabs like a binder: drop blocks inside, one tab shows at a time', size: [6, 4] },
  { type: 'sharing_button', label: 'Sharing button', hint: 'Sends everything to a connection', size: [4, 2] },
];

export const typeLabel = (t: BlockType) => BLOCK_TYPES.find((b) => b.type === t)?.label ?? t;

export function overlaps(a: Rect, b: Rect): boolean {
  return a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
}

export function fits(r: Rect, cols = COLUMNS): boolean {
  return r.w >= 1 && r.h >= 1 && r.x >= 0 && r.y >= 0 && r.x + r.w <= cols;
}

const inside = (r: Rect, outer: Rect) =>
  r.x >= outer.x && r.y >= outer.y && r.x + r.w <= outer.x + outer.w && r.y + r.h <= outer.y + outer.h;

/** Where a block would go: in a tab (`{ tabs, index }`), at the top level (`null`), or nowhere (`undefined`). */
type Spot = { tabs: string; index: number } | null | undefined;

/**
 * Can a block (`selfId`, or a new block of `type`) take `rect`? Inside a Tabs block it joins the
 * open tab: it may cover the Tabs block and the blocks of its other tabs, not those of its own.
 * Moving a Tabs block takes its blocks along, so they are not in its way.
 */
function spotFor(blocks: readonly Block[], rect: Rect, selfId: string | undefined, type: BlockType | undefined, cols: number, open: OpenTabs): Spot {
  if (!fits(rect, cols)) return undefined;
  const self = selfId ? blocks.find((b) => b.id === selfId) : undefined;
  const isTabs = (self?.type ?? type) === 'tabs';
  if (isTabs && (rect.w < MIN_TABS_SIZE || rect.h < MIN_TABS_SIZE)) return undefined;
  const along = new Set(self?.type === 'tabs' ? childIds(self) : []);
  const holder = isTabs ? undefined : blocks.find((b): b is TabsBlock => b.type === 'tabs' && b.id !== selfId && inside(rect, b.rect));
  // a block moved inside its own Tabs block stays in its tab; one brought in joins the open tab
  const own = selfId ? tabOf(blocks, selfId) : null;
  const index = holder ? (own && own.tabs.id === holder.id ? own.index : openTab(holder, open)) : -1;
  for (const b of blocks) {
    if (b.id === selfId || along.has(b.id) || !overlaps(b.rect, rect)) continue;
    if (holder) {
      if (b.id === holder.id) continue;
      const t = tabOf(blocks, b.id);
      if (t && t.tabs.id === holder.id && t.index !== index) continue;
    }
    return undefined;
  }
  return holder ? { tabs: holder.id, index } : null;
}

export function canPlace(
  blocks: readonly Block[], rect: Rect, ignoreId?: string, cols = COLUMNS, opts: { open?: OpenTabs; type?: BlockType } = {},
): boolean {
  return spotFor(blocks, rect, ignoreId, opts.type, cols, opts.open ?? {}) !== undefined;
}

/** The blocks with `id` in the tab `spot` names, and in no other (top level when `null`). */
function joinTab(blocks: readonly Block[], id: string, spot: { tabs: string; index: number } | null): Block[] {
  return blocks.map((b) => {
    if (b.type !== 'tabs') return b;
    const tabs = b.tabs.map((t, i) => {
      const rest = (t.blocks ?? []).filter((x) => x !== id);
      const mine = spot && spot.tabs === b.id && spot.index === i ? [...rest, id] : rest;
      return mine.length ? { ...t, blocks: mine } : { name: t.name };
    });
    return { ...b, tabs };
  });
}

/** Rows to draw: everything placed, plus a spare row, never fewer than [`MIN_ROWS`]. */
export function rowsNeeded(blocks: readonly Block[]): number {
  const bottom = blocks.reduce((m, b) => Math.max(m, b.rect.y + b.rect.h), 0);
  return Math.max(MIN_ROWS, bottom + 2);
}

/** The first spot (reading order) where a `w` by `h` block fits. Always exists: rows are unbounded. */
export function firstFree(blocks: readonly Block[], w: number, h: number, cols = COLUMNS): Rect {
  const width = Math.min(Math.max(1, w), cols);
  for (let y = 0; ; y++) {
    for (let x = 0; x + width <= cols; x++) {
      const r = { x, y, w: width, h };
      if (canPlace(blocks, r, undefined, cols)) return r;
    }
  }
}

const ID_PREFIX: Record<BlockType, string> = {
  screenshot: 'shot',
  video: 'clip',
  text: 'text',
  dropdown: 'dropdown',
  files: 'files',
  tags: 'tags',
  log_view: 'log',
  tabs: 'tabs',
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
    case 'tags':
      return { type, title: 'Tags', tags: [] };
    case 'log_view':
      return { type, title: 'Log' };
    case 'tabs':
      return { type, tabs: [{ name: 'Tab 1' }, { name: 'Tab 2' }] };
    case 'sharing_button':
      return {
        type, label: 'Share', connection_id: null, prompt_block_id: null, mappings: [], on_success: 'close_window',
      };
  }
}

/** A new block of `type` placed at `at` (or the first free spot), with a fresh id. */
export function makeBlock(
  blocks: readonly Block[], type: BlockType, at?: { x: number; y: number }, cols = COLUMNS, open: OpenTabs = {},
): Block | null {
  return addBlock(blocks, type, at, cols, open)?.block ?? null;
}

/** The blocks with a new one of `type` (in the open tab when it is dropped inside a Tabs block). */
export function addBlock(
  blocks: readonly Block[], type: BlockType, at?: { x: number; y: number }, cols = COLUMNS, open: OpenTabs = {},
): { blocks: Block[]; block: Block } | null {
  if (type === 'sharing_button' && blocks.filter((b) => b.type === 'sharing_button').length >= MAX_SHARING_BUTTONS) return null;
  const [w, h] = BLOCK_TYPES.find((b) => b.type === type)!.size;
  let rect: Rect;
  let spot: Spot = null;
  if (at) {
    const x = Math.min(Math.max(0, at.x), cols - w);
    rect = { x, y: Math.max(0, at.y), w, h };
    spot = spotFor(blocks, rect, undefined, type, cols, open);
    if (spot === undefined) return null;
  } else {
    rect = firstFree(blocks, w, h, cols);
  }
  const block = { id: newBlockId(blocks, type), rect, ...content(type) } as Block;
  const all = [...blocks, block];
  return { blocks: spot ? joinTab(all, block.id, spot) : all, block };
}

/** The block at `rect`, in the tab it now lies in; a Tabs block takes its blocks along. */
function replaceRect(blocks: readonly Block[], id: string, rect: Rect, cols: number, open: OpenTabs): Block[] | null {
  const self = blocks.find((b) => b.id === id);
  if (!self) return null;
  const spot = spotFor(blocks, rect, id, undefined, cols, open);
  if (spot === undefined) return null;
  if (self.type === 'tabs') {
    const along = new Set(childIds(self));
    const dx = rect.x - self.rect.x;
    const dy = rect.y - self.rect.y;
    // resized: its blocks must still be inside it
    if ((rect.w !== self.rect.w || rect.h !== self.rect.h) && blocks.some((b) => along.has(b.id) && !inside(b.rect, rect))) return null;
    return blocks.map((b) =>
      b.id === id ? { ...b, rect } : along.has(b.id) && (dx || dy) ? { ...b, rect: { ...b.rect, x: b.rect.x + dx, y: b.rect.y + dy } } : b,
    );
  }
  return joinTab(blocks.map((b) => (b.id === id ? { ...b, rect } : b)), id, spot);
}

/** Move to a cell, keeping the block inside the grid. `null` when something is in the way. */
export function moveTo(blocks: readonly Block[], id: string, x: number, y: number, cols = COLUMNS, open: OpenTabs = {}): Block[] | null {
  const b = blocks.find((k) => k.id === id);
  if (!b) return null;
  const nx = Math.min(Math.max(0, x), cols - b.rect.w);
  return replaceRect(blocks, id, { ...b.rect, x: nx, y: Math.max(0, y) }, cols, open);
}

/** Resize, at least one cell, within the grid. `null` when something is in the way. */
export function resizeTo(blocks: readonly Block[], id: string, w: number, h: number, cols = COLUMNS, open: OpenTabs = {}): Block[] | null {
  const b = blocks.find((k) => k.id === id);
  if (!b) return null;
  const nw = Math.min(Math.max(1, w), cols - b.rect.x);
  return replaceRect(blocks, id, { ...b.rect, w: nw, h: Math.max(1, h) }, cols, open);
}

/**
 * Remove a block and everything that pointed at it: a sharing button's prompt block, any
 * mapping that took its value from the block, and its place in a tab. Stale references would only show up later as
 * confusing warnings.
 */
export function removeBlock(blocks: readonly Block[], id: string): Block[] {
  const self = blocks.find((b) => b.id === id);
  // a Tabs block goes with the blocks in its tabs
  const inner = self?.type === 'tabs' ? childIds(self) : [];
  const rest = inner.reduce<Block[]>((acc, child) => removeBlock(acc, child), [...blocks]);
  return joinTab(rest, id, null)
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
  px: number, py: number, gridWidth: number, rowHeight: number, gap: number, cols = COLUMNS,
): { x: number; y: number } {
  const cellW = (gridWidth - gap * (cols - 1)) / cols;
  return {
    x: Math.floor(px / (cellW + gap)),
    y: Math.floor(py / (rowHeight + gap)),
  };
}

/** A copy of the layout with its blocks in reading order, so saved files diff cleanly. */
export function tidy(layout: Layout): Layout {
  const blocks = [...layout.blocks].sort((a, b) => a.rect.y - b.rect.y || a.rect.x - b.rect.x);
  const out: Layout = { ...layout, blocks };
  // 12 is what a layout has when it does not say
  if (out.columns === undefined || columnsOf(out) === COLUMNS) delete out.columns;
  else out.columns = columnsOf(out);
  return out;
}
