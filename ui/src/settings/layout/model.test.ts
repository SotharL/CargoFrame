import { describe, expect, it } from 'vitest';
import type { Block, Layout } from '../../lib/types';
import {
  canPlace, cellAt, COLUMNS, firstFree, fits, makeBlock, MAX_SHARING_BUTTONS, moveTo, newBlockId, overlaps, removeBlock,
  resizeTo, rowsNeeded, tidy,
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
