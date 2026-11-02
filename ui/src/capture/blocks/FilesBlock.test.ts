import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import FilesBlock from './FilesBlock.svelte';
import type { Block, FilesState } from '../../lib/types';

type Files = Extract<Block, { type: 'files' }>;
const block: Files = { id: 'files', type: 'files', rect: { x: 0, y: 0, w: 6, h: 3 }, title: 'Files', include: ['**'], preselect: [] };
const info = (n: number): FilesState => ({
  entries: Array.from({ length: n }, (_, i) => ({ path: i % 2 ? `logs/run-${i}.log` : `src/file-${i}.ts`, size: 100 * i, selected: false })),
  truncated: false,
  problem: null,
});

describe('FilesBlock', () => {
  afterEach(cleanup);

  it('lists every file, however many there are, in one list element that scrolls', () => {
    render(FilesBlock, { block, info: info(60), selected: [] });
    expect(screen.getAllByRole('checkbox')).toHaveLength(60);
    expect(document.querySelectorAll('ul')).toHaveLength(1);
  });

  it('offers a filter only for a longer list', () => {
    const { unmount } = render(FilesBlock, { block, info: info(4), selected: [] });
    expect(screen.queryByRole('textbox', { name: 'Filter files' })).toBeNull();
    unmount();
    render(FilesBlock, { block, info: info(20), selected: [] });
    expect(screen.getByRole('textbox', { name: 'Filter files' })).toBeTruthy();
  });

  it('filters, and All / None act on what is shown', async () => {
    render(FilesBlock, { block, info: info(20), selected: ['src/file-0.ts'] });
    await fireEvent.input(screen.getByRole('textbox', { name: 'Filter files' }), { target: { value: '.log' } });
    expect(screen.getAllByRole('checkbox')).toHaveLength(10);
    expect(screen.getByText(/1 of 20 selected · 10 shown/)).toBeTruthy();

    await fireEvent.click(screen.getByRole('button', { name: 'All' }));
    expect(screen.getByText(/11 of 20 selected/)).toBeTruthy();

    await fireEvent.click(screen.getByRole('button', { name: 'None' }));
    // Only the shown ones were cleared: the one hidden by the filter is still ticked.
    expect(screen.getByText(/1 of 20 selected/)).toBeTruthy();
  });

  it('says so when the filter matches nothing', async () => {
    render(FilesBlock, { block, info: info(20), selected: [] });
    await fireEvent.input(screen.getByRole('textbox', { name: 'Filter files' }), { target: { value: 'zzz' } });
    expect(screen.getByText('No file matches the filter.')).toBeTruthy();
  });
});
