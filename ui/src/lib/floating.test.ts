import { describe, expect, it } from 'vitest';
import { placeUnder } from './floating';

const box = (top: number, left = 20, width = 200) => ({ top, bottom: top + 30, left, width });

describe('a list that opens under a box', () => {
  it('opens under it, at least wide enough to read', () => {
    expect(placeUnder(box(100), 1000, 800)).toEqual({ left: 20, top: 132, width: 260, maxH: 240, up: false });
  });

  it('opens over it near the bottom of the window, and stays on screen', () => {
    const p = placeUnder(box(700, 900), 1000, 800);
    expect(p.up).toBe(true);
    expect(p.top).toBe(698);
    expect(p.left + p.width).toBeLessThanOrEqual(992);
  });
});
