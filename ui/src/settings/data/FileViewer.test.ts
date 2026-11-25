import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import FileViewer from './FileViewer.svelte';

afterEach(cleanup);

const file = { id: 'a.log', name: 'a.log', modified_ms: 0, lines: ['LogAI: Spawned 30 crew'], cut: false, problem: null };
const marks = (s: number, e: number) => ({ sel: { file: 'a.log', line: 0, s, e }, anchor: '', found: [], kept: [] });

describe('the grips of a selection', () => {
  it('sit at its two ends and move it a character with the arrow keys', async () => {
    const onadjust = vi.fn();
    render(FileViewer, { files: [file], current: 'a.log', marks: marks(7, 14), onselect: () => {}, onadjust, onopen: () => {} });
    const start = screen.getByRole('slider', { name: 'Start of the selection' });
    const end = screen.getByRole('slider', { name: 'End of the selection' });
    // nothing is added to the text: the line reads the same
    expect(document.querySelector('.text')!.textContent).toBe('LogAI: Spawned 30 crew');
    expect(start.nextElementSibling?.textContent).toBe('Spawned');
    await fireEvent.keyDown(end, { key: 'ArrowRight' });
    expect(onadjust).toHaveBeenLastCalledWith('a.log', 0, 7, 15);
    await fireEvent.keyDown(start, { key: 'ArrowLeft' });
    expect(onadjust).toHaveBeenLastCalledWith('a.log', 0, 6, 14);
  });

  it('never makes it empty, and are not drawn without a way to adjust it', async () => {
    const onadjust = vi.fn();
    render(FileViewer, { files: [file], current: 'a.log', marks: marks(7, 8), onselect: () => {}, onadjust, onopen: () => {} });
    await fireEvent.keyDown(screen.getByRole('slider', { name: 'End of the selection' }), { key: 'ArrowLeft' });
    expect(onadjust).not.toHaveBeenCalled();
    cleanup();
    render(FileViewer, { files: [file], current: 'a.log', marks: marks(7, 14), onselect: () => {}, onopen: () => {} });
    expect(screen.queryByRole('slider')).toBeNull();
  });
});
