import { afterEach, describe, expect, it, vi } from 'vitest';
import { drawMark, markFontSize, markRadius } from './drawMark';

/** A canvas that writes down what is drawn on it (jsdom has none). */
function fakeCanvas(png = true) {
  const calls: [string, ...unknown[]][] = [];
  const ctx = new Proxy(
    {},
    {
      get: (_t, name: string) => (...args: unknown[]) => calls.push([name, ...args]),
      set: (_t, name: string, value) => {
        calls.push(['set ' + name, value]);
        return true;
      },
    },
  );
  const canvas = {
    width: 0,
    height: 0,
    getContext: () => ctx,
    toBlob: (cb: (b: Blob | null) => void) => cb(png ? new Blob(['x'], { type: 'image/png' }) : null),
  };
  vi.spyOn(document, 'createElement').mockImplementation(((tag: string) => (tag === 'canvas' ? canvas : document.createElementNS('http://www.w3.org/1999/xhtml', tag))) as never);
  return { calls, canvas };
}
afterEach(() => vi.restoreAllMocks());
const named = (calls: [string, ...unknown[]][], n: string) => calls.filter((c) => c[0] === n);

describe('drawMark', () => {
  it('draws a ring the size of the picture with its text under it', async () => {
    const { calls, canvas } = fakeCanvas();
    const blob = await drawMark(1280, 720, { x: 400, y: 300 }, '  Look here  ');
    expect(canvas.width).toBe(1280);
    expect(canvas.height).toBe(720);
    expect(blob.type).toBe('image/png');
    expect(named(calls, 'arc')[0]).toEqual(['arc', 400, 300, markRadius(1280, 720), 0, Math.PI * 2]);
    const text = named(calls, 'fillText')[0]!;
    expect(text[1]).toBe('Look here');
    expect(text[2]).toBe(400);
    // under the ring, not on it
    expect(text[3] as number).toBeGreaterThan(300 + markRadius(1280, 720));
    expect(named(calls, 'strokeText')).toHaveLength(1);
  });

  it('draws no text for an empty one', async () => {
    const { calls } = fakeCanvas();
    await drawMark(640, 360, { x: 10, y: 10 }, '   ');
    expect(named(calls, 'arc')).toHaveLength(1);
    expect(named(calls, 'fillText')).toHaveLength(0);
  });

  it('keeps the ring and its text inside the picture', async () => {
    const { calls } = fakeCanvas();
    await drawMark(640, 360, { x: 9999, y: 9999 }, 'Corner');
    const arc = named(calls, 'arc')[0]!;
    expect(arc[1]).toBe(640);
    expect(arc[2]).toBe(360);
    const y = named(calls, 'fillText')[0]![3] as number;
    expect(y).toBeLessThanOrEqual(360 - markFontSize(360) * 1.2);
  });

  it('says so when the picture cannot be made', async () => {
    fakeCanvas(false);
    await expect(drawMark(640, 360, { x: 1, y: 1 }, '')).rejects.toMatch(/could not be made/);
  });
});
