import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import MarkLayer from './MarkLayer.svelte';
import Harness from './MarkLayer.harness.svelte';
import { containRect } from './contain';

// jsdom does no layout: the space the picture is shown in is 400 x 400 here, and the picture is
// 800 x 400, so it fills the width and sits in the middle (a 400 x 200 picture, 100 px down).
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

describe('containRect', () => {
  it('shows a picture whole, centred, never stretched', () => {
    expect(containRect({ w: 400, h: 400 }, 800, 400)).toEqual({ left: 0, top: 100, w: 400, h: 200 });
    expect(containRect({ w: 400, h: 100 }, 800, 400)).toEqual({ left: 100, top: 0, w: 200, h: 100 });
    expect(containRect({ w: 0, h: 100 }, 800, 400)).toEqual({ left: 0, top: 0, w: 0, h: 0 });
  });
});

describe('MarkLayer', () => {
  const ring = () => screen.getByRole('group', { name: /Mark at/ });

  it('shows the ring where the mark is, with its text under it', () => {
    render(MarkLayer, { width: 800, height: 400, at: { x: 400, y: 200 }, text: 'Look here' });
    expect(ring().style.left).toBe('50%');
    expect(ring().style.top).toBe('50%');
    // a tenth of the shorter side in radius, shown at half size
    expect(ring().style.width).toBe('40px');
    expect(screen.getByText('Look here')).toBeTruthy();
  });

  it('moves the mark to the place that is clicked, and ignores a click beside the picture', async () => {
    const seen: { x: number; y: number }[] = [];
    const { container } = render(Harness, { onat: (a: { x: number; y: number }) => seen.push(a) });
    const layer = container.querySelector('.layer') as HTMLElement;
    layer.getBoundingClientRect = () => ({ left: 0, top: 0, width: 400, height: 400, right: 400, bottom: 400, x: 0, y: 0, toJSON: () => ({}) });
    // 100 px right, 150 px down: 25% across, (150 - 100) / 200 = 25% down
    await fireEvent.click(layer, { clientX: 100, clientY: 150 });
    expect(seen[seen.length - 1]).toEqual({ x: 200, y: 100 });
    // above the picture (it starts 100 px down)
    await fireEvent.click(layer, { clientX: 100, clientY: 40 });
    expect(seen[seen.length - 1]).toEqual({ x: 200, y: 100 });
  });

  it('moves with the arrow keys and stays inside the picture', async () => {
    const seen: { x: number; y: number }[] = [];
    render(Harness, { onat: (a: { x: number; y: number }) => seen.push(a) });
    await fireEvent.keyDown(ring(), { key: 'ArrowRight' });
    expect(seen[seen.length - 1]).toEqual({ x: 410, y: 200 });
    await fireEvent.keyDown(ring(), { key: 'ArrowUp', shiftKey: true });
    expect(seen[seen.length - 1]).toEqual({ x: 410, y: 150 });
    for (let i = 0; i < 40; i++) await fireEvent.keyDown(ring(), { key: 'ArrowLeft', shiftKey: true });
    expect(seen[seen.length - 1]!.x).toBe(0);
    for (let i = 0; i < 40; i++) await fireEvent.keyDown(ring(), { key: 'ArrowDown', shiftKey: true });
    expect(seen[seen.length - 1]!.y).toBe(400);
  });
});
