import { afterEach, describe, expect, it, vi } from 'vitest';
import { drawMarks } from './drawMark';

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

describe('drawMarks', () => {
  it('draws every mark on a see-through picture the size of the recording', async () => {
    const { calls, canvas } = fakeCanvas();
    const blob = await drawMarks(1280, 720, [
      { kind: 'ellipse', color: '#e04f4f', width: 6, a: { x: 300, y: 200 }, b: { x: 500, y: 400 } },
      { kind: 'arrow', color: '#62e04f', width: 4, a: { x: 10, y: 10 }, b: { x: 200, y: 100 } },
      { kind: 'text', color: '#ffffff', size: 40, at: { x: 50, y: 600 }, text: 'Look here', box: true },
    ]);
    expect(canvas.width).toBe(1280);
    expect(canvas.height).toBe(720);
    expect(blob.type).toBe('image/png');
    expect(named(calls, 'ellipse')[0]).toEqual(['ellipse', 400, 300, 100, 100, 0, 0, Math.PI * 2]);
    expect(named(calls, 'fillText')[0]!.slice(1, 4)).toEqual(['Look here', 50, 600]);
    // the arrow's head is a filled triangle
    expect(named(calls, 'fill').length).toBeGreaterThanOrEqual(2);
  });

  it('says so when the picture cannot be made', async () => {
    fakeCanvas(false);
    await expect(drawMarks(10, 10, [])).rejects.toMatch(/could not/);
  });
});
