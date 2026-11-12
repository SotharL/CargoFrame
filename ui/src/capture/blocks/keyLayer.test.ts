import { afterEach, describe, expect, it, vi } from 'vitest';
import { drawKeys, keyPixels, lineAt, placeAt } from './keyLayer';

afterEach(() => vi.restoreAllMocks());

describe('keyLayer', () => {
  const lines = [
    { from_ms: 1_000, to_ms: 2_000, text: 'a' },
    { from_ms: 2_000, to_ms: 3_500, text: 'ab' },
  ];

  it('finds the line showing at a moment', () => {
    expect(lineAt(lines, 500)).toBeNull();
    expect(lineAt(lines, 1_000)?.text).toBe('a');
    expect(lineAt(lines, 2_000)?.text).toBe('ab');
    expect(lineAt(lines, 3_500)).toBeNull();
    expect(lineAt(undefined, 1_000)).toBeNull();
  });

  it('slides a following crop from place to place, on even pixels', () => {
    const places = [
      { at_ms: 1_000, x: 10, y: 10 },
      { at_ms: 2_000, x: 110, y: 10 },
      { at_ms: 2_000, x: 200, y: 50 },
    ];
    expect(placeAt(places, 0)).toEqual({ x: 10, y: 10 });
    expect(placeAt(places, 1_500)).toEqual({ x: 60, y: 10 });
    expect(placeAt(places, 1_510)).toEqual({ x: 62, y: 10 });
    expect(placeAt(places, 5_000)).toEqual({ x: 200, y: 50 });
    expect(placeAt([], 5)).toEqual({ x: 0, y: 0 });
  });

  it('draws a blank picture then one per line, all the size of the widest, black on white', async () => {
    const calls: [string, ...unknown[]][] = [];
    const ctx = new Proxy({} as Record<string, unknown>, {
      get: (_t, name: string) =>
        name === 'measureText' ? (t: string) => ({ width: t.length * 10 }) : (...args: unknown[]) => calls.push([name, ...args]),
      set: (_t, name: string, value) => {
        calls.push(['set ' + name, value]);
        return true;
      },
    });
    const canvas = { width: 0, height: 0, getContext: () => ctx, toBlob: (cb: (b: Blob) => void) => cb(new Blob([String(canvas.width)])) };
    vi.spyOn(document, 'createElement').mockImplementation((() => canvas) as never);
    const pictures = await drawKeys(lines, 720);
    expect(pictures).toHaveLength(3);
    const px = keyPixels(720);
    expect(px).toBe(40);
    // as wide as "ab" (20) with the outline either side
    expect(canvas.width).toBe(20 + 7 * 2 + 4);
    expect(calls.filter((c) => c[0] === 'fillText').map((c) => c[1])).toEqual(['a', 'ab']);
    expect(calls).toContainEqual(['set strokeStyle', '#fff']);
    expect(calls).toContainEqual(['set fillStyle', '#000']);
  });

  it('keeps the letters readable on a small picture', () => {
    expect(keyPixels(100)).toBe(12);
  });
});
