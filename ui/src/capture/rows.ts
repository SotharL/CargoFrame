import type { Block } from '../lib/types';
import { childIds, tabOf, type TabsBlock } from '../lib/tabs';

/** The least height of a row of the window's grid. */
export const ROW = 44;
/** A title above a box, and one line of a box (its padding and border too). */
const TITLE = 22;
const LINE = 40;
const TOOLBAR = 32;
/** The space between two rows (App.svelte's grid gap). */
const GAP = 10;
/** A Tabs block's names (on the top or bottom), and the padding and border of its page. */
const TAB_NAMES = 30;
const PAGE = 18;

/** The height a block needs to show at least one line of what it holds; 0 when any will do. */
export function needs(b: Block, toolbar: boolean): number {
  switch (b.type) {
    case 'text':
      return (b.placement === 'inside' ? 0 : TITLE) + (toolbar ? TOOLBAR : 0) + LINE;
    case 'dropdown':
    case 'tags':
      return TITLE + LINE;
    case 'sharing_button':
      return LINE;
    default:
      return 0;
  }
}

/**
 * The least height of each of `rows` rows: ROW, or more where a block a row high (or its share,
 * when it spans several) needs it. `toolbar` says whether a Text block shows formatting buttons.
 * The blocks in tabs count through their Tabs block, whose page holds their rows.
 */
export function rowMinimums(blocks: readonly Block[], rows: number, toolbar: (b: Block) => boolean): number[] {
  const out = Array.from({ length: rows }, () => ROW);
  for (const b of blocks) {
    if (tabOf(blocks, b.id)) continue;
    const need = b.type === 'tabs' ? tabsNeed(b, blocks, toolbar) : needs(b, toolbar(b));
    // a block over several rows has the gaps between them too
    const share = Math.ceil((need - GAP * (b.rect.h - 1)) / b.rect.h);
    for (let y = b.rect.y; y < b.rect.y + b.rect.h && y < rows; y++) out[y] = Math.max(out[y]!, share);
  }
  return out;
}

/** The blocks of a Tabs block's tabs, placed from its own top-left corner. */
export function insideTabs(t: TabsBlock, blocks: readonly Block[]): Block[] {
  const ids = new Set(childIds(t));
  return blocks
    .filter((b) => ids.has(b.id))
    .map((b) => ({ ...b, rect: { ...b.rect, x: b.rect.x - t.rect.x, y: b.rect.y - t.rect.y } }));
}

/** The least height of each row of a Tabs block's page. */
export const tabRows = (t: TabsBlock, blocks: readonly Block[], toolbar: (b: Block) => boolean) =>
  rowMinimums(insideTabs(t, blocks), t.rect.h, toolbar);

function tabsNeed(t: TabsBlock, blocks: readonly Block[], toolbar: (b: Block) => boolean): number {
  const across = (t.side ?? 'top') === 'top' || t.side === 'bottom';
  const inner = tabRows(t, blocks, toolbar);
  return (across ? TAB_NAMES : 0) + PAGE + inner.reduce((a, h) => a + h, 0) + GAP * (t.rect.h - 1);
}
