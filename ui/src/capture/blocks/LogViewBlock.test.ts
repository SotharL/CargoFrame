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
    expect(screen.getByText(/^2 of 6 lines with “\[ERROR\]”/)).toBeTruthy();
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

describe('LogViewBlock by time', () => {
  afterEach(cleanup);
  const local = (h: number, mi: number, s: number) => new Date(2026, 10, 12, h, mi, s).getTime();
  const text = ['12:00:00 boot', '12:00:10 [info] load', '12:00:20 [error] crash', '  at Foo.bar', '12:00:30 [info] quit'].join('\n');
  const api = () => memoryApi({ readOfferedFile: () => ({ text, size: text.length, cut: false }) });
  async function recorded(): Promise<SessionView> {
    const v = await view();
    // recorded from 12:00:18 for 4 s
    v.video = { width: 640, height: 360, duration_ms: 4_000, bytes: 1, audio: false, started_at_ms: local(12, 0, 18) };
    v.captured_at_ms = local(12, 0, 22);
    return v;
  }
  const shownLines = (list: HTMLElement) => within(list).getAllByRole('button').map((b) => b.textContent?.replace(/^\d+/, '').trim());

  it('switches the search row to the time, which keeps the lines written during the recording', async () => {
    render(LogViewBlock, { block, view: await recorded(), files: { logs: ['run.log'] }, api: api() });
    const list = await screen.findByRole('list', { name: 'Lines' });
    expect(within(list).getAllByRole('button')).toHaveLength(5);
    // one row: the text search, until the clock is asked for
    expect(screen.queryByRole('combobox', { name: 'Lines written' })).toBeNull();
    // the button shows the clock: what it turns the row to
    const button = screen.getByRole('button', { name: 'Search by time' });
    await fireEvent.click(button);
    // now it shows the magnifier, to come back to the words
    expect(button.getAttribute('aria-label')).toBe('Search by text');
    expect(screen.queryByRole('searchbox')).toBeNull();
    const when = screen.getByRole('combobox', { name: 'Lines written' }) as HTMLSelectElement;
    expect([...when.options].map((o) => o.text)).toEqual(['Written any time', 'During the recording (12:00:13 to 12:00:27)', 'Written between…']);
    // it starts on the recording, a few seconds either side, with the rest of each message
    expect(when.value).toBe('moment');
    await waitFor(() => expect(shownLines(list)).toEqual(['12:00:20 [error] crash', 'at Foo.bar']));
    expect(screen.getByText(/2 of 5 lines, written 12:00:13 to 12:00:27/)).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Copy 2 lines' })).toBeTruthy();
    // back to the words: the time keeps applying, and the button says so with a dot
    await fireEvent.click(button);
    expect(button.getAttribute('aria-label')).toBe('Search by time');
    expect(button.classList.contains('other')).toBe(true);
    await fireEvent.input(screen.getByRole('searchbox'), { target: { value: 'foo' } });
    await waitFor(() => expect(shownLines(list)).toEqual(['at Foo.bar']));
    expect(screen.getByText(/1 of 5 lines with “foo”, written 12:00:13/)).toBeTruthy();
  });

  it('keeps the lines between two times typed, starting on the recording', async () => {
    render(LogViewBlock, { block, view: await recorded(), files: { logs: ['run.log'] }, api: api() });
    const list = await screen.findByRole('list', { name: 'Lines' });
    await fireEvent.click(screen.getByRole('button', { name: 'Search by time' }));
    await fireEvent.change(screen.getByRole('combobox', { name: 'Lines written' }), { target: { value: 'between' } });
    const from = screen.getByLabelText('From') as HTMLInputElement;
    const to = screen.getByLabelText('To') as HTMLInputElement;
    expect([from.value, to.value]).toEqual(['12:00:13', '12:00:27']);
    await fireEvent.input(from, { target: { value: '12:00:00' } });
    await fireEvent.input(to, { target: { value: '12:00:10' } });
    await waitFor(() => expect(shownLines(list)).toEqual(['12:00:00 boot', '12:00:10 [info] load']));
  });

  it('reads the times as UTC when the block says, and starts on the recording when asked', async () => {
    const offset = new Date(local(12, 0, 0)).getTimezoneOffset();
    render(LogViewBlock, { block: { ...block, utc: true, during: true }, view: await recorded(), files: { logs: ['run.log'] }, api: api() });
    const list = await screen.findByRole('list', { name: 'Lines' });
    // asked to start on the recording, the row still starts on the text search; the time
    // search applies, and the clock says so
    expect(screen.getByRole('searchbox')).toBeTruthy();
    const clock = screen.getByRole('button', { name: 'Search by time' });
    expect(clock.classList.contains('other')).toBe(true);
    await fireEvent.click(clock);
    expect((screen.getByRole('combobox', { name: 'Lines written' }) as HTMLSelectElement).value).toBe('moment');
    const utc = screen.getByRole('checkbox', { name: 'UTC' }) as HTMLInputElement;
    expect(utc.checked).toBe(true);
    // in UTC, 12:00:20 is this computer's 12:00:20 only where the two are the same
    await waitFor(() => expect(within(list).queryAllByRole('button').length > 0 || offset !== 0).toBe(true));
    await fireEvent.click(utc);
    await waitFor(() => expect(shownLines(list)).toEqual(['12:00:20 [error] crash', 'at Foo.bar']));
  });

  it('has the search switch even when a file has no times, and says so', async () => {
    const plain = memoryApi({ readOfferedFile: () => ({ text: 'a\nb', size: 3, cut: false }) });
    render(LogViewBlock, { block: { ...block, during: true }, view: await recorded(), files: { logs: ['run.log'] }, api: plain });
    const list = await screen.findByRole('list', { name: 'Lines' });
    expect(within(list).getAllByRole('button')).toHaveLength(2);
    // the search bar comes first; the clock is there all the same, and says the file has no
    // times while every line shows
    expect(screen.getByRole('searchbox')).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: 'Search by time' }));
    expect(screen.getByText('No times in this file')).toBeTruthy();
    expect(screen.getByText(/no times CargoFrame can read in this file/)).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: 'Search by text' }));
    expect(screen.getByRole('searchbox')).toBeTruthy();
  });
});
