import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render } from '@testing-library/svelte';
import Harness from './CropBox.harness.svelte';

// The space is 400 x 400 and the picture 800 x 400: it is shown 400 wide, so 2 picture pixels per CSS pixel.
beforeEach(() => {
  vi.stubGlobal('ResizeObserver', class { constructor(private cb: () => void) {} observe() { this.cb(); } disconnect() {} });
  Object.defineProperty(HTMLElement.prototype, 'clientWidth', { configurable: true, get: () => 400 });
  Object.defineProperty(HTMLElement.prototype, 'clientHeight', { configurable: true, get: () => 400 });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  delete (HTMLElement.prototype as unknown as Record<string, unknown>).clientWidth;
  delete (HTMLElement.prototype as unknown as Record<string, unknown>).clientHeight;
});

describe('CropBox with the mouse', () => {
  const box = { x: 200, y: 100, w: 400, h: 200 };

  it('moves with the box that is dragged, in the picture’s own pixels, inside the picture', async () => {
    const seen: (typeof box)[] = [];
    const { container } = render(Harness, { start: box, oncrop: (c: typeof box) => seen.push(c) });
    const el = container.querySelector('.box') as HTMLElement;
    await fireEvent.mouseDown(el, { clientX: 100, clientY: 100 });
    // 20 CSS px right and 10 down: 40 and 20 picture pixels
    await fireEvent.mouseMove(window, { clientX: 120, clientY: 110 });
    expect(seen[seen.length - 1]).toEqual({ x: 240, y: 120, w: 400, h: 200 });
    // far to the right: it stops at the edge of the picture
    await fireEvent.mouseMove(window, { clientX: 1000, clientY: 110 });
    expect(seen[seen.length - 1]!.x).toBe(800 - 400);
    await fireEvent.mouseUp(window);
    await fireEvent.mouseMove(window, { clientX: 0, clientY: 0 });
    expect(seen[seen.length - 1]!.x).toBe(400);
  });

  it('is resized by its corner, never below 16 pixels or past the picture, and stays even', async () => {
    const seen: (typeof box)[] = [];
    const { container } = render(Harness, { start: box, oncrop: (c: typeof box) => seen.push(c) });
    const grip = container.querySelector('.grip') as HTMLElement;
    await fireEvent.mouseDown(grip, { clientX: 0, clientY: 0 });
    await fireEvent.mouseMove(window, { clientX: 10, clientY: -5 });
    expect(seen[seen.length - 1]).toEqual({ x: 200, y: 100, w: 420, h: 190 });
    await fireEvent.mouseMove(window, { clientX: -5000, clientY: -5000 });
    expect(seen[seen.length - 1]).toMatchObject({ w: 16, h: 16 });
    await fireEvent.mouseMove(window, { clientX: 5000, clientY: 5000 });
    expect(seen[seen.length - 1]).toMatchObject({ w: 600, h: 300 });
    await fireEvent.mouseUp(window);
  });
});
