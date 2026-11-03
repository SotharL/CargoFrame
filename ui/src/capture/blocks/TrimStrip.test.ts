import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import TrimStrip from './TrimStrip.svelte';

afterEach(cleanup);

const start = () => screen.getByRole('slider', { name: 'Start of the part to keep' });
const end = () => screen.getByRole('slider', { name: 'End of the part to keep' });
const value = (el: HTMLElement) => Number(el.getAttribute('aria-valuenow'));

describe('TrimStrip', () => {
  it('puts the two handles where the kept part begins and ends', () => {
    render(TrimStrip, { duration: 10, start: 2, end: 8 });
    expect(start().style.left).toBe('20%');
    expect(end().style.left).toBe('80%');
    expect(end().getAttribute('aria-valuetext')).toBe('8.0 seconds');
  });

  it('moves a handle a tenth of a second with the arrows, a second with Shift', async () => {
    render(TrimStrip, { duration: 10, start: 2, end: 8 });
    await fireEvent.keyDown(start(), { key: 'ArrowRight' });
    expect(value(start())).toBe(2.1);
    await fireEvent.keyDown(start(), { key: 'ArrowRight', shiftKey: true });
    expect(value(start())).toBe(3.1);
    await fireEvent.keyDown(end(), { key: 'ArrowLeft' });
    expect(value(end())).toBe(7.9);
    await fireEvent.keyDown(start(), { key: 'ArrowLeft', shiftKey: true });
    expect(value(start())).toBe(2.1);
  });

  it('never keeps less than half a second, and the ends cannot pass each other', async () => {
    render(TrimStrip, { duration: 10, start: 4, end: 5 });
    await fireEvent.keyDown(start(), { key: 'ArrowRight', shiftKey: true });
    expect(value(start())).toBe(4.5);
    await fireEvent.keyDown(start(), { key: 'ArrowRight' });
    expect(value(start())).toBe(4.5);
    await fireEvent.keyDown(end(), { key: 'ArrowLeft', shiftKey: true });
    expect(value(end())).toBe(5);
  });

  it('goes to the very start and the very end with Home and End', async () => {
    render(TrimStrip, { duration: 9.47, start: 2, end: 8 });
    await fireEvent.keyDown(start(), { key: 'Home' });
    expect(value(start())).toBe(0);
    await fireEvent.keyDown(end(), { key: 'End' });
    // the length is not a whole number of tenths, and the end is the end
    expect(value(end())).toBe(9.47);
  });

  it('takes the picture to the place that is clicked, and a click on a handle does not', async () => {
    const onseek = vi.fn();
    const { container } = render(TrimStrip, { duration: 10, start: 0, end: 10, onseek });
    const strip = container.querySelector('.strip') as HTMLElement;
    strip.getBoundingClientRect = () => ({ left: 100, width: 200, top: 0, right: 300, bottom: 20, height: 20, x: 100, y: 0, toJSON: () => ({}) });
    await fireEvent.click(strip, { clientX: 150 });
    expect(onseek).toHaveBeenCalledWith(2.5);
    await fireEvent.click(start(), { clientX: 150 });
    expect(onseek).toHaveBeenCalledTimes(1);
  });

  it('follows the mouse while a handle is dragged, and stops when it is let go', async () => {
    const { container } = render(TrimStrip, { duration: 10, start: 0, end: 10 });
    const strip = container.querySelector('.strip') as HTMLElement;
    strip.getBoundingClientRect = () => ({ left: 0, width: 200, top: 0, right: 200, bottom: 20, height: 20, x: 0, y: 0, toJSON: () => ({}) });
    await fireEvent.mouseDown(end(), { clientX: 200 });
    await fireEvent.mouseMove(window, { clientX: 100 });
    expect(value(end())).toBe(5);
    await fireEvent.mouseUp(window);
    await fireEvent.mouseMove(window, { clientX: 20 });
    expect(value(end())).toBe(5);
    // dragging the start past the end stops half a second short of it
    await fireEvent.mouseDown(start(), { clientX: 0 });
    await fireEvent.mouseMove(window, { clientX: 190 });
    expect(value(start())).toBe(4.5);
    await fireEvent.mouseUp(window);
  });

  it('shows the playing position', () => {
    const { container } = render(TrimStrip, { duration: 10, start: 0, end: 10, now: 2.5 });
    expect((container.querySelector('.playhead') as HTMLElement).style.left).toBe('25%');
  });
});
