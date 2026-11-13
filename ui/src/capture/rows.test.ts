import { describe, expect, it } from 'vitest';
import type { Block } from '../lib/types';
import { ROW, rowMinimums, tabRows } from './rows';

const text = (y: number, h: number, placement: 'above' | 'inside' = 'above') =>
  ({ id: `t${y}`, type: 'text', rect: { x: 0, y, w: 4, h }, title: 'T', placement, role: 'plain', default_text: '' }) as Block;

describe('row heights', () => {
  it('gives a row the height its one-line blocks need, and no more elsewhere', () => {
    const shot = { id: 'shot', type: 'screenshot', rect: { x: 4, y: 0, w: 8, h: 3 } } as Block;
    const rows = rowMinimums([shot, text(0, 1), text(1, 1, 'inside'), text(2, 1)], 4, (b) => b.id === 't2');
    // a title and a line; a line alone fits a plain row; a title, a toolbar and a line
    expect(rows).toEqual([62, ROW, 94, ROW]);
  });

  it('shares what a taller block needs between its rows, gaps counted', () => {
    expect(rowMinimums([text(0, 2)], 2, () => true)).toEqual([ROW, ROW]);
  });

  it('gives a Tabs block room for its names and the rows of its page', () => {
    const tabs = { id: 'tabs', type: 'tabs', rect: { x: 0, y: 0, w: 6, h: 2 }, tabs: [{ name: 'A', blocks: ['t0'] }, { name: 'B' }] } as Block;
    const inner = text(0, 1);
    expect(tabRows(tabs as Extract<Block, { type: 'tabs' }>, [tabs, inner], () => false)).toEqual([62, ROW]);
    // names 30 + page 18 + 62 + 44 + gap 10 = 164, over two rows and a gap
    expect(rowMinimums([tabs, inner], 2, () => false)).toEqual([77, 77]);
  });
});
