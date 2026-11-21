import { describe, expect, it } from 'vitest';
import type { Block, Layout } from '../../lib/types';
import {
  addBlock, canPlace, cellAt, COLUMNS, firstFree, fits, makeBlock, MAX_SHARING_BUTTONS, moveTo, newBlockId, overlaps, removeBlock,
  resizeTo, rowsNeeded, tidy, columnsOf, usedColumns, copyOf, pasteBlocks, isLocked, lockTab,
} from './model';

const text = (id: string, x: number, y: number, w = 4, h = 2): Block => ({
  id, rect: { x, y, w, h }, type: 'text', title: id, placement: 'above', role: 'plain', default_text: '',
});

describe('grid rules', () => {
  it('overlap is by cells; touching edges do not overlap', () => {
    expect(overlaps({ x: 0, y: 0, w: 4, h: 2 }, { x: 4, y: 0, w: 4, h: 2 })).toBe(false);
    expect(overlaps({ x: 0, y: 0, w: 4, h: 2 }, { x: 3, y: 1, w: 4, h: 2 })).toBe(true);
  });

  it('a rect must be inside 12 columns and at least one cell', () => {
    expect(fits({ x: 8, y: 0, w: 4, h: 1 })).toBe(true);
    expect(fits({ x: 9, y: 0, w: 4, h: 1 })).toBe(false);
    expect(fits({ x: 0, y: 0, w: 0, h: 1 })).toBe(false);
    expect(fits({ x: -1, y: 0, w: 1, h: 1 })).toBe(false);
  });

  it('finds the first free spot in reading order, growing downward', () => {
    expect(firstFree([], 4, 2)).toEqual({ x: 0, y: 0, w: 4, h: 2 });
    const blocks = [text('a', 0, 0, 12, 2)];
    expect(firstFree(blocks, 4, 2)).toEqual({ x: 0, y: 2, w: 4, h: 2 });
    const gap = [text('a', 0, 0, 4, 2), text('b', 8, 0, 4, 2)];
    expect(firstFree(gap, 4, 2)).toEqual({ x: 4, y: 0, w: 4, h: 2 });
    expect(firstFree(gap, 6, 2).y).toBe(2);
    expect(firstFree([], 99, 1).w).toBe(COLUMNS);
  });

  it('shows a spare row below the blocks and never fewer than ten', () => {
    expect(rowsNeeded([])).toBe(10);
    expect(rowsNeeded([text('a', 0, 12, 4, 3)])).toBe(17);
  });
});

describe('editing', () => {
  it('moves within the grid, clamping, and refuses a move onto another block', () => {
    const blocks = [text('a', 0, 0), text('b', 4, 0)];
    const moved = moveTo(blocks, 'a', 20, 5)!;
    expect(moved.find((b) => b.id === 'a')!.rect).toEqual({ x: 8, y: 5, w: 4, h: 2 });
    expect(moveTo(blocks, 'a', 3, 0)).toBeNull();
    expect(moveTo(blocks, 'a', -4, -4)!.find((b) => b.id === 'a')!.rect).toMatchObject({ x: 0, y: 0 });
    expect(moveTo(blocks, 'zzz', 0, 0)).toBeNull();
  });

  it('resizes to at least one cell and no further than the edge, or refuses', () => {
    const blocks = [text('a', 0, 0), text('b', 6, 0)];
    expect(resizeTo(blocks, 'a', 6, 3)!.find((b) => b.id === 'a')!.rect).toMatchObject({ w: 6, h: 3 });
    expect(resizeTo(blocks, 'a', 7, 2)).toBeNull();
    expect(resizeTo(blocks, 'a', 0, 0)!.find((b) => b.id === 'a')!.rect).toMatchObject({ w: 1, h: 1 });
    expect(resizeTo([text('c', 10, 0, 2, 1)], 'c', 9, 1)!.at(0)!.rect.w).toBe(2);
  });

  it('can always place into free space and refuses an occupied cell', () => {
    const blocks = [text('a', 0, 0)];
    expect(canPlace(blocks, { x: 4, y: 0, w: 4, h: 2 })).toBe(true);
    expect(canPlace(blocks, { x: 2, y: 0, w: 4, h: 2 })).toBe(false);
    expect(canPlace(blocks, { x: 2, y: 0, w: 4, h: 2 }, 'a')).toBe(true);
  });
});

describe('new blocks', () => {
  it('gets a fresh id and a sensible size, in the first free spot', () => {
    const a = makeBlock([], 'screenshot')!;
    expect(a.id).toBe('shot');
    expect(a.rect).toEqual({ x: 0, y: 0, w: 8, h: 8 });
    const t1 = makeBlock([a], 'text')!;
    expect(t1.id).toBe('text-1');
    expect(t1.rect).toEqual({ x: 8, y: 0, w: 4, h: 3 });
    expect(makeBlock([a, t1], 'text')!.id).toBe('text-2');
    // a video block is named like the screenshot, the first one plainly
    const clip = makeBlock([a, t1], 'video')!;
    expect(clip.id).toBe('clip');
    expect(clip.type).toBe('video');
    expect(makeBlock([a, t1, clip], 'video')!.id).toBe('clip-1');
    expect(newBlockId([text('text-1', 0, 0)], 'text')).toBe('text-2');
  });

  it('dropped at a cell it lands there, clamped to the edge, or is refused when occupied', () => {
    const a = text('a', 0, 0);
    expect(makeBlock([a], 'dropdown', { x: 11, y: 4 })!.rect).toEqual({ x: 8, y: 4, w: 4, h: 2 });
    expect(makeBlock([a], 'dropdown', { x: 1, y: 0 })).toBeNull();
  });

  it('every type starts valid and empty of connections', () => {
    for (const t of ['screenshot', 'text', 'dropdown', 'files', 'sharing_button'] as const) {
      const b = makeBlock([], t)!;
      expect(fits(b.rect)).toBe(true);
    }
    const share = makeBlock([], 'sharing_button')!;
    expect(share).toMatchObject({ connection_id: null, prompt_block_id: null, mappings: [] });
  });
});

describe('removing', () => {
  it('drops references to the removed block', () => {
    const share: Block = {
      id: 'send', rect: { x: 8, y: 0, w: 4, h: 2 }, type: 'sharing_button', label: 'Share',
      connection_id: 'claude', prompt_block_id: 'prompt', on_success: 'keep_open',
      mappings: [
        { field: 'summary', source: { kind: 'block', value: 'prompt' } },
        { field: 'priority', source: { kind: 'rule', value: 'prompt' } },
        { field: 'labels', source: { kind: 'fixed', value: 'prompt' } },
      ],
    };
    const left = removeBlock([text('prompt', 0, 0), share], 'prompt');
    expect(left).toHaveLength(1);
    const s = left[0] as Extract<Block, { type: 'sharing_button' }>;
    expect(s.prompt_block_id).toBeNull();
    // Only the mapping that read from the block goes; a rule or fixed value that merely shares
    // the text is not a reference to it.
    expect(s.mappings.map((m) => m.field)).toEqual(['priority', 'labels']);
  });
});

describe('pointer to cell', () => {
  it('maps pixels to cells for a given grid size', () => {
    // 12 columns in 1200px with 8px gaps: cells are (1200 - 88) / 12 = 92.67 wide
    expect(cellAt(0, 0, 1200, 44, 8)).toEqual({ x: 0, y: 0 });
    expect(cellAt(101, 53, 1200, 44, 8)).toEqual({ x: 1, y: 1 });
    expect(cellAt(1199, 0, 1200, 44, 8).x).toBe(11);
  });
});

describe('tidy', () => {
  it('orders blocks in reading order for clean diffs', () => {
    const layout: Layout = { id: 'l', name: 'L', blocks: [text('c', 4, 2), text('a', 0, 0), text('b', 4, 0)] };
    expect(tidy(layout).blocks.map((b) => b.id)).toEqual(['a', 'b', 'c']);
    expect(layout.blocks[0]!.id).toBe('c');
  });
});

describe('the limit of sharing buttons', () => {
  it('adds sharing buttons up to the limit of a window and refuses the next one', () => {
    let blocks: Block[] = [];
    for (let i = 0; i < MAX_SHARING_BUTTONS; i++) {
      const b = makeBlock(blocks, 'sharing_button');
      expect(b).not.toBeNull();
      blocks = [...blocks, b!];
    }
    expect(MAX_SHARING_BUTTONS).toBe(5);
    expect(makeBlock(blocks, 'sharing_button')).toBeNull();
    expect(makeBlock(blocks, 'sharing_button', { x: 0, y: 20 })).toBeNull();
    // other blocks are not counted against it
    expect(makeBlock(blocks, 'text')).not.toBeNull();
    // removing one makes room again
    expect(makeBlock(removeBlock(blocks, blocks[0]!.id), 'sharing_button')).not.toBeNull();
  });
});

describe('a wider grid', () => {
  const at = (x: number, w: number): Block => ({ id: `b${x}`, type: 'screenshot', rect: { x, y: 0, w, h: 1 } });

  it('has room on the side for a block past column 12', () => {
    expect(canPlace([], { x: 12, y: 0, w: 4, h: 1 })).toBe(false);
    expect(canPlace([], { x: 12, y: 0, w: 4, h: 1 }, undefined, 16)).toBe(true);
    expect(moveTo([at(0, 4)], 'b0', 20, 0, 16)?.[0]?.rect.x).toBe(12);
    expect(resizeTo([at(0, 4)], 'b0', 30, 1, 16)?.[0]?.rect.w).toBe(16);
  });

  it('keeps the column count within 12 to 24 and writes none for 12', () => {
    expect(columnsOf({})).toBe(12);
    expect(columnsOf({ columns: 30 })).toBe(24);
    expect(columnsOf({ columns: 3 })).toBe(12);
    expect(usedColumns([at(0, 4), at(10, 6)])).toBe(16);
    const base: Layout = { id: 'l', name: 'L', blocks: [] };
    expect('columns' in tidy({ ...base, columns: 12 })).toBe(false);
    expect(tidy({ ...base, columns: 18 }).columns).toBe(18);
  });
});

describe('tabs, like a binder', () => {
  const tabs = (blocks: string[][] = [[], []]): Block => ({
    id: 'tabs', rect: { x: 0, y: 0, w: 8, h: 4 }, type: 'tabs', tabs: blocks.map((b, i) => (b.length ? { name: `T${i + 1}`, blocks: b } : { name: `T${i + 1}` })),
  });
  const tabsOf = (blocks: Block[]) => (blocks.find((b) => b.id === 'tabs') as Extract<Block, { type: 'tabs' }>).tabs.map((t) => t.blocks ?? []);

  it('a block dropped inside goes in the open tab, and blocks of other tabs share its cells', () => {
    const one = addBlock([tabs()], 'text', { x: 0, y: 1 })!;
    expect(tabsOf(one.blocks)).toEqual([[one.block.id], []]);
    // the same cells, in the second tab
    expect(canPlace(one.blocks, one.block.rect, undefined, COLUMNS, { open: { tabs: 1 } })).toBe(true);
    const two = addBlock(one.blocks, 'text', { x: 0, y: 1 }, COLUMNS, { tabs: 1 })!;
    expect(tabsOf(two.blocks)).toEqual([[one.block.id], [two.block.id]]);
    // but not in the first again, nor across the tabs' edge
    expect(canPlace(two.blocks, one.block.rect)).toBe(false);
    expect(canPlace(two.blocks, { x: 6, y: 0, w: 4, h: 2 })).toBe(false);
  });

  it('moving a block out of the tabs takes it out of its tab', () => {
    const blocks = [tabs([['a'], []]), text('a', 0, 0)];
    const next = moveTo(blocks, 'a', 0, 5)!;
    expect(tabsOf(next)).toEqual([[], []]);
    // and back in, to the tab open then
    expect(tabsOf(moveTo(next, 'a', 2, 1, COLUMNS, { tabs: 1 })!)).toEqual([[], ['a']]);
    // moved inside, it stays in its own tab, whichever is open
    expect(tabsOf(moveTo(blocks, 'a', 1, 1, COLUMNS, { tabs: 1 })!)).toEqual([['a'], []]);
  });

  it('the tabs take their blocks along, keep them inside when resized, and go with them', () => {
    const blocks = [tabs([['a'], ['b']]), text('a', 0, 0), text('b', 4, 2)];
    const moved = moveTo(blocks, 'tabs', 2, 3)!;
    expect(moved.map((b) => [b.id, b.rect.x, b.rect.y])).toEqual([['tabs', 2, 3], ['a', 2, 3], ['b', 6, 5]]);
    expect(resizeTo(blocks, 'tabs', 6, 4)).toBeNull(); // b would be outside
    expect(resizeTo(blocks, 'tabs', 1, 4)).toBeNull(); // at least 2 by 2
    expect(resizeTo(blocks, 'tabs', 8, 5)).not.toBeNull();
    expect(removeBlock(blocks, 'tabs')).toEqual([]);
    expect(tabsOf(removeBlock(blocks, 'b'))).toEqual([['a'], []]);
  });

  it('tabs do not go inside tabs', () => {
    expect(canPlace([tabs()], { x: 0, y: 0, w: 4, h: 2 }, undefined, COLUMNS, { type: 'tabs' })).toBe(false);
  });
});


describe('copy, paste, duplicate', () => {
  type Tabs = Extract<Block, { type: 'tabs' }>;
  type Button = Extract<Block, { type: 'sharing_button' }>;
  const button = (id: string, from: string): Button => ({
    id, rect: { x: 8, y: 0, w: 4, h: 1 }, type: 'sharing_button', label: 'Share', connection_id: 'c', prompt_block_id: from,
    on_success: 'close_window', mappings: [{ field: 'summary', source: { kind: 'block', value: from } }], required: [from],
  });

  it('pastes a copy with a new id at the first free place, the original left as it is', () => {
    const blocks = [text('text-1', 0, 0)];
    const done = pasteBlocks(blocks, copyOf(blocks, 'text-1'))!;
    expect(done.blocks).toHaveLength(2);
    expect(done.block.id).toBe('text-2');
    expect(done.block.rect).toEqual({ x: 4, y: 0, w: 4, h: 2 });
    expect(done.blocks[0]).toEqual(blocks[0]);
  });

  it('copies a Tabs block with the blocks of its tabs, all renamed and moved together', () => {
    const tabs: Tabs = { id: 'tabs', rect: { x: 0, y: 0, w: 6, h: 4 }, type: 'tabs', tabs: [{ name: 'A', blocks: ['in'] }, { name: 'B', locked: true }] };
    const blocks = [tabs, text('in', 1, 1, 3, 2)];
    const done = pasteBlocks(blocks, copyOf(blocks, 'tabs'))!;
    const copy = done.block as Tabs;
    expect(copy.id).toBe('tabs-1');
    expect(copy.rect).toEqual({ x: 6, y: 0, w: 6, h: 4 });
    const inner = copy.tabs[0]!.blocks![0]!;
    expect(inner).not.toBe('in');
    // inside the copy, where it was inside the original
    expect(done.blocks.find((b) => b.id === inner)!.rect).toEqual({ x: 7, y: 1, w: 3, h: 2 });
    expect(copy.tabs[1]!.locked).toBe(true);
  });

  it('a pasted button names the copied blocks it named, and the others as they were', () => {
    const blocks = [text('note', 0, 0), button('share', 'note')];
    const alone = pasteBlocks(blocks, copyOf(blocks, 'share'))!.block as Button;
    // copied alone: it still sends the note
    expect(alone.prompt_block_id).toBe('note');
    expect(alone.mappings[0]!.source.value).toBe('note');
    expect(alone.required).toEqual(['note']);
  });

  it('never pastes one sharing button too many', () => {
    let blocks: Block[] = [];
    for (let i = 0; i < MAX_SHARING_BUTTONS; i++) blocks = addBlock(blocks, 'sharing_button')!.blocks;
    expect(pasteBlocks(blocks, copyOf(blocks, blocks[0]!.id))).toBeNull();
  });
});

describe('a locked tab', () => {
  type Tabs = Extract<Block, { type: 'tabs' }>;
  const layout = (): Block[] => [
    { id: 'tabs', rect: { x: 0, y: 0, w: 8, h: 4 }, type: 'tabs', tabs: [{ name: 'A', blocks: ['in'] }, { name: 'B' }] } as Tabs,
    text('in', 0, 1, 3, 2),
    text('out', 0, 5),
  ];

  it('keeps its blocks where they are and takes no other, until it is unlocked', () => {
    const locked = lockTab(layout(), 'tabs', 0, true);
    expect(isLocked(locked, 'in')).toBe(true);
    expect(isLocked(locked, 'out')).toBe(false);
    expect(moveTo(locked, 'in', 4, 1)).toBeNull();
    expect(resizeTo(locked, 'in', 4, 2)).toBeNull();
    // nothing is dropped in it, moved or new
    expect(moveTo(locked, 'out', 4, 1, COLUMNS, { tabs: 0 })).toBeNull();
    expect(addBlock(locked, 'text', { x: 4, y: 1 }, COLUMNS, { tabs: 0 })).toBeNull();
    // the other tab is not locked
    expect(addBlock(locked, 'text', { x: 4, y: 1 }, COLUMNS, { tabs: 1 })).not.toBeNull();
    // the Tabs block itself still moves, its blocks with it
    expect(moveTo(locked, 'tabs', 2, 0)?.find((b) => b.id === 'in')?.rect.x).toBe(2);
    const open = lockTab(locked, 'tabs', 0, false);
    expect((open[0] as Tabs).tabs[0]).toEqual({ name: 'A', blocks: ['in'] });
    expect(moveTo(open, 'in', 4, 1)).not.toBeNull();
  });
});
