import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import LogViewBlock from './LogViewBlock.svelte';
import { matching, splitLines } from './logLines';
import { memoryApi } from '../../lib/ipc';
import type { Block, SessionView } from '../../lib/types';
import { demoSession } from '../demo';

type Log = Extract<Block, { type: 'log_view' }>;
const block: Log = { id: 'log', type: 'log_view', rect: { x: 0, y: 0, w: 6, h: 4 }, title: 'Log', filter: '' };

async function view(): Promise<SessionView> {
  const v = await demoSession();
  v.files = { logs: { entries: [{ path: 'run.log', size: 10, selected: true }, { path: 'other.txt', size: 4, selected: false }], truncated: false, problem: null, places: [] } };
  return v;
}

describe('log lines', () => {
  it('are numbered, and the search keeps the lines with every word, case aside', () => {
    const lines = splitLines('a [ERROR] x\r\nb [info]\nc [error] disk\n');
    expect(lines.map((l) => l.n)).toEqual([1, 2, 3]);
    expect(matching(lines, '[error]').map((l) => l.n)).toEqual([1, 3]);
    expect(matching(lines, 'error  DISK').map((l) => l.text)).toEqual(['c [error] disk']);
    expect(matching(lines, '  ')).toHaveLength(3);
  });
});

describe('LogViewBlock', () => {
  afterEach(cleanup);

  it('shows the first log, filters its lines, shows a line whole to copy, and copies the lines shown', async () => {
    const copied: string[] = [];
    Object.assign(navigator, { clipboard: { writeText: async (t: string) => void copied.push(t) } });
    render(LogViewBlock, { block, view: await view(), files: { logs: ['run.log'] }, api: memoryApi() });
    const list = await screen.findByRole('list', { name: 'Lines' });
    expect(within(list).getAllByRole('button')).toHaveLength(6);
    await fireEvent.input(screen.getByRole('searchbox', { name: 'Show only the lines with these words' }), { target: { value: '[ERROR]' } });
    await waitFor(() => expect(within(list).getAllByRole('button')).toHaveLength(2));
    expect(screen.getByText('2 of 6 lines')).toBeTruthy();
    expect(list.querySelectorAll('mark')[0]!.textContent).toBe('[error]');
    await fireEvent.click(within(list).getAllByRole('button')[1]!);
    const line = screen.getByRole('group', { name: 'Line 6' });
    expect(line.textContent).toContain('Save failed again');
    await fireEvent.click(within(line).getByRole('button', { name: 'Copy line' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Copy 2 lines' }));
    expect(copied).toEqual(['12:00:11 [error] Save failed again', '12:00:09 [error] Save failed: disk full\n12:00:11 [error] Save failed again']);
    // a second file can be picked; Open large fills the window
    expect(screen.getByRole('combobox', { name: 'File to show' })).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: 'Open large' }));
    expect(screen.getByRole('region', { name: 'Log' }).classList.contains('large')).toBe(true);
  });

  it('starts with the filter of the layout, and says when there is nothing to show', async () => {
    render(LogViewBlock, { block: { ...block, filter: 'warning' }, view: await view(), files: { logs: [] }, api: memoryApi() });
    const list = await screen.findByRole('list', { name: 'Lines' });
    expect(within(list).getAllByRole('button')).toHaveLength(1);
    cleanup();
    const empty = await view();
    empty.files = {};
    render(LogViewBlock, { block, view: empty, files: {}, api: memoryApi() });
    expect(screen.getByText(/No file to show/)).toBeTruthy();
  });
});
