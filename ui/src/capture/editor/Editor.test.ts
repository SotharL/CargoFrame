import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import Editor from './Editor.svelte';

beforeEach(() => {
  vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} });
  // jsdom draws nothing; the toolbar is what is under test
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null);
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const open = () =>
  render(Editor, { image: {} as CanvasImageSource, original: new Blob(['x']), size: { width: 800, height: 400 } });

describe('Editor toolbar', () => {
  it('is made of icon buttons, each with a name and a tooltip, so it stays on one line', () => {
    open();
    const bar = screen.getByRole('toolbar', { name: 'Screenshot tools' });
    const buttons = [...bar.querySelectorAll('button')].filter((b) => !b.closest('.panel'));
    const names = buttons.map((b) => b.getAttribute('aria-label'));
    expect(names).toEqual(['Crop', 'Draw', 'Square', 'Circle', 'Text', 'Colour and thickness', 'Undo', 'Redo', 'Reset crop', 'Clear all']);
    for (const b of buttons) {
      expect(b.textContent?.trim()).toBe('');
      expect(b.getAttribute('title')).toBeTruthy();
    }
  });

  it('starts with nothing to undo, redo or clear, and picks a tool with one click', async () => {
    open();
    for (const name of ['Undo', 'Redo', 'Reset crop', 'Clear all']) {
      expect((screen.getByRole('button', { name }) as HTMLButtonElement).disabled).toBe(true);
    }
    const draw = screen.getByRole('button', { name: 'Draw' });
    await fireEvent.click(draw);
    expect(draw.getAttribute('aria-pressed')).toBe('true');
    await fireEvent.click(draw);
    expect(draw.getAttribute('aria-pressed')).toBe('false');
  });

  it('keeps the colours and thicknesses in the panel of the one style button', async () => {
    open();
    expect(screen.queryByRole('button', { name: /^Colour #/ })).toBeNull();
    await fireEvent.click(screen.getByRole('button', { name: 'Colour and thickness' }));
    expect(screen.getAllByRole('button', { name: /^Colour #/ })).toHaveLength(8);
    expect(screen.getByRole('button', { name: 'thin' })).toBeTruthy();
  });
});
