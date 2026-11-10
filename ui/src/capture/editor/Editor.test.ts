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
    expect(names).toEqual(['Crop', 'Draw', 'Square', 'Circle', 'Arrow', 'Text', 'Colour and thickness', 'Undo', 'Redo', 'Reset crop', 'Clear all']);
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
  it('writes text where it is clicked, styled, and opens a text again to change it', async () => {
    open();
    await fireEvent.click(screen.getByRole('button', { name: 'Text' }));
    const canvas = document.querySelector('canvas')!;
    await fireEvent.pointerDown(canvas, { button: 0, clientX: 10, clientY: 10, pointerId: 1 });
    const box = (await screen.findByRole('textbox', { name: 'Text to add' })) as HTMLTextAreaElement;
    // the style buttons do not take the focus, so the box stays open
    await fireEvent.click(screen.getByRole('button', { name: 'Italic' }));
    expect(screen.getByRole('button', { name: 'Italic' }).getAttribute('aria-pressed')).toBe('true');
    await fireEvent.input(box, { target: { value: 'Crash here' } });
    await fireEvent.keyDown(box, { key: 'Enter' });
    expect(screen.queryByRole('textbox', { name: 'Text to add' })).toBeNull();
    expect((screen.getByRole('button', { name: 'Undo' }) as HTMLButtonElement).disabled).toBe(false);
    // a click on it brings it back to be changed
    await fireEvent.pointerDown(canvas, { button: 0, clientX: 10, clientY: 10, pointerId: 1 });
    expect(((await screen.findByRole('textbox', { name: 'Text to add' })) as HTMLTextAreaElement).value).toBe('Crash here');
  });
});

describe('Editor read text', () => {
  const reading = (attempt: number) => ({
    text: 'Build failed\nat line 4',
    lines: [
      { text: 'Build failed', x: 2, y: 2, w: 60, h: 10 },
      { text: 'at line 4', x: 2, y: 16, w: 50, h: 10 },
    ],
    reader: 'tesseract',
    attempt,
  });

  function drag(canvas: HTMLElement) {
    canvas.getBoundingClientRect = () => ({ left: 0, top: 0, width: 800, height: 400, right: 800, bottom: 400, x: 0, y: 0, toJSON() {} });
    canvas.setPointerCapture = () => {};
    canvas.releasePointerCapture = () => {};
    fireEvent.pointerDown(canvas, { button: 0, clientX: 100, clientY: 100, pointerId: 1 });
    fireEvent.pointerMove(canvas, { clientX: 300, clientY: 160, pointerId: 1 });
    fireEvent.pointerUp(canvas, { clientX: 300, clientY: 160, pointerId: 1 });
  }

  it('is offered only when the app can read text', () => {
    open();
    expect(screen.queryByRole('button', { name: 'Read text' })).toBeNull();
  });

  it('reads the part dragged over, shows its lines, copies them, and tries another way', async () => {
    const ocr = await import('./ocr');
    vi.spyOn(ocr, 'regionPng').mockResolvedValue(new Blob(['png']));
    const reader = vi.fn(async (_png: Blob, attempt: number) => reading(attempt));
    const writeText = vi.fn(async () => {});
    vi.stubGlobal('navigator', { ...navigator, clipboard: { writeText } });
    render(Editor, { image: {} as CanvasImageSource, original: new Blob(['x']), size: { width: 800, height: 400 }, reader });
    await fireEvent.click(screen.getByRole('button', { name: 'Read text' }));
    drag(screen.getByLabelText(/^Screenshot,/));
    await vi.waitFor(() => expect(screen.getByRole('textbox', { name: 'The text found' })).toBeTruthy());
    expect(reader).toHaveBeenCalledWith(expect.any(Blob), 0);
    expect(ocr.regionPng).toHaveBeenCalledWith(expect.anything(), { x: 100, y: 100, w: 200, h: 60 });
    expect((screen.getByRole('textbox', { name: 'The text found' }) as HTMLTextAreaElement).value).toBe('Build failed\nat line 4');
    // each line is marked where it was found, and can be picked to copy alone
    const line = screen.getByRole('button', { name: 'Line 2: at line 4' });
    // (the frame is drawn at a tenth of the picture's size here: jsdom has no room)
    expect(line.style.left).toBe('10.2px');
    expect(line.style.top).toBe('11.6px');
    await fireEvent.click(line);
    await fireEvent.click(screen.getByRole('button', { name: 'Copy 1 line' }));
    expect(writeText).toHaveBeenCalledWith('at line 4');
    expect(screen.getByRole('button', { name: 'Copied' })).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: 'Try again' }));
    await vi.waitFor(() => expect(reader).toHaveBeenLastCalledWith(expect.any(Blob), 1));
    await vi.waitFor(() => expect(screen.getByText(/way 2 of 3/)).toBeTruthy());
    await fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(screen.queryByRole('region', { name: 'Text read' })).toBeNull();
  });

  it('says what went wrong, and can try again', async () => {
    const ocr = await import('./ocr');
    vi.spyOn(ocr, 'regionPng').mockResolvedValue(new Blob(['png']));
    const reader = vi.fn(async () => {
      throw new Error('To read text on Linux, install tesseract');
    });
    render(Editor, { image: {} as CanvasImageSource, original: new Blob(['x']), size: { width: 800, height: 400 }, reader });
    await fireEvent.click(screen.getByRole('button', { name: 'Read text' }));
    drag(screen.getByLabelText(/^Screenshot,/));
    await vi.waitFor(() => expect(screen.getByRole('alert').textContent).toContain('install tesseract'));
    expect(screen.getByRole('button', { name: 'Try again' })).toBeTruthy();
  });
});
