import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import DataFields from './DataFields.svelte';
import { memoryApi, setApiForTests, type Api } from '../../lib/ipc';
import type { Rule } from '../../lib/types';
import { go } from '../route.svelte';

let api: Api;
beforeEach(() => {
  api = memoryApi();
  setApiForTests(api);
  go('rules', 'values');
});
afterEach(() => {
  cleanup();
  setApiForTests(undefined);
});

const saved = async () => (await api.listItems<Rule>('rule')).items;

/** Click (or drag, with `to`) on the line of the shown file that holds `text`, at `word`. */
async function selectIn(text: string, word: string, to?: string) {
  const row = await waitFor(() => {
    const r = [...document.querySelectorAll('[data-line]')].find((e) => e.textContent?.includes(text));
    expect(r).toBeTruthy();
    return r!;
  });
  const node = row.querySelector('.text span')!.firstChild!;
  const line = node.textContent!;
  const r = document.createRange();
  r.setStart(node, line.indexOf(word) + 1);
  r.setEnd(node, to === undefined ? line.indexOf(word) + 1 : line.indexOf(to) + to.length);
  const sel = window.getSelection()!;
  sel.removeAllRanges();
  sel.addRange(r);
  await fireEvent.mouseUp(row);
}

describe('Data & fields: values selected in a file', () => {
  it('makes a value by clicking it in a log, read in every log that has it, the newest winning', async () => {
    render(DataFields);
    await fireEvent.click(await screen.findByRole('button', { name: '＋ New value: select it in a file' }));
    await fireEvent.click(await screen.findByRole('tab', { name: 'Game.log' }));
    expect(screen.getByText('Nothing selected yet')).toBeTruthy();

    await selectIn('Harbor_P?listen', 'Harbor_P');
    const name = screen.getByRole('textbox', { name: 'Name of the value' }) as HTMLInputElement;
    expect(name.value).toBe('Maps');
    await fireEvent.input(name, { target: { value: 'Map' } });
    expect(screen.getByText('lines with “Maps/”')).toBeTruthy();
    expect(screen.getByRole('radio', { name: 'at the next “?”' }).getAttribute('aria-checked')).toBe('true');
    expect(screen.getByText(/In Game\.log: found on 2 lines, gives “Harbor_P”/)).toBeTruthy();

    // both logs have it: the editor's log changed last, so it is the one used
    const reads = screen.getByRole('heading', { name: 'Read it in these files' }).parentElement!;
    await waitFor(() => expect([...reads.querySelectorAll('.read .value')].map((e) => e.textContent?.trim())).toEqual(['Harbor_Night_P', 'Harbor_P', 'Not found in this file']));
    expect((within(reads).getByRole('checkbox', { name: /Editor\.log/ }) as HTMLInputElement).checked).toBe(true);
    expect((within(reads).getByRole('checkbox', { name: /system_info/ }) as HTMLInputElement).checked).toBe(false);
    expect(reads.querySelector('.gives .what')?.textContent).toBe('Harbor_Night_P');
    expect([...reads.querySelectorAll('.used.on')]).toHaveLength(1);
    await fireEvent.click(screen.getByRole('radio', { name: 'all, when they differ' }));
    expect(reads.querySelector('.gives .what')?.textContent).toBe('Harbor_Night_P, Harbor_P');
    await fireEvent.click(screen.getByRole('radio', { name: 'the newest file' }));

    await fireEvent.click(screen.getByRole('button', { name: 'Save {map}' }));
    await waitFor(async () => expect((await saved()).length).toBe(2));
    const rules = await saved();
    expect(rules.map((r) => r.item.file).sort()).toEqual(['Editor.log', 'Game.log']);
    expect(rules[0]!.item.values![0]).toMatchObject({
      key: 'map', name: 'Map', which: 'last', across: 'newest', find: { type: 'pick', lines_with: 'Maps/', from_line_start: false, stop: '?' },
    });

    // the list shows it with what the last capture gave
    const row = (await screen.findByText('{map}')).closest('tr')!;
    expect(within(row).getByText('Editor.log, Game.log')).toBeTruthy();
    expect(within(row).getByText('2 files · the newest wins')).toBeTruthy();
    await waitFor(() => expect(within(row).getByText('Harbor_Night_P')).toBeTruthy());
  });

  it('refuses a name a value already has, whatever its case', async () => {
    await api.saveItem('rule', 'game', { name: 'Game.log', file: 'Game.log', values: [{ key: 'map', name: 'Map', find: { type: 'pick', lines_with: 'Maps/', stop: '?' }, which: 'last', fallback: null }] } as never);
    render(DataFields);
    await fireEvent.click(await screen.findByRole('button', { name: '＋ New value: select it in a file' }));
    await fireEvent.click(await screen.findByRole('tab', { name: 'Game.log' }));
    await selectIn('Spawned 12 crew', 'Spawned', 'Harbor_P');
    expect(screen.getByText('for “Spawned 12 crew on Harbor_P”')).toBeTruthy();
    expect(screen.getByRole('radio', { name: 'at the end of the line' }).getAttribute('aria-checked')).toBe('true');
    const name = screen.getByRole('textbox', { name: 'Name of the value' });
    await fireEvent.input(name, { target: { value: 'MAP' } });
    expect(screen.getByRole('alert').textContent).toBe('{map} already exists.');
    expect((screen.getByRole('button', { name: 'Save {map}' }) as HTMLButtonElement).disabled).toBe(true);
    await fireEvent.input(name, { target: { value: 'Crew' } });
    expect((screen.getByRole('button', { name: 'Save {crew}' }) as HTMLButtonElement).disabled).toBe(false);
  });

  it('opens a value to see each file it is read in, and settles several files the same way everywhere', async () => {
    const step = { key: 'map', name: 'Map', find: { type: 'pick', lines_with: 'Maps/', stop: '?' }, which: 'last', fallback: null };
    await api.saveItem('rule', 'game', { name: 'Game.log', file: 'Game.log', values: [step] } as never);
    await api.saveItem('rule', 'editor', { name: 'Editor.log', file: 'Editor.log', values: [step] } as never);
    render(DataFields);
    await fireEvent.click(await screen.findByRole('button', { name: 'Open Map' }));
    expect(await screen.findByRole('heading', { name: 'Read in' })).toBeTruthy();
    await waitFor(() => expect(screen.getByText('Harbor_Night_P')).toBeTruthy());
    await fireEvent.click(screen.getByRole('radio', { name: 'all, when they differ' }));
    await waitFor(async () => expect((await saved()).every((r) => r.item.values![0]!.across === 'all')).toBe(true));
    await fireEvent.click(screen.getByRole('radio', { name: 'write a text' }));
    await fireEvent.input(screen.getByRole('textbox', { name: 'Text when no file has it' }), { target: { value: 'unknown' } });
    await fireEvent.change(screen.getByRole('textbox', { name: 'Text when no file has it' }));
    await waitFor(async () => expect((await saved()).every((r) => r.item.values![0]!.fallback === 'unknown')).toBe(true));
    // no longer read in the game's log: its rule had nothing else to do
    await fireEvent.click(screen.getByRole('button', { name: 'Stop reading it in Game.log' }));
    await waitFor(async () => expect((await saved()).map((r) => r.id)).toEqual(['editor']));
  });

  it('lists the files, and keeps values made before 0.5.0 working', async () => {
    await api.saveItem('rule', 'sys', {
      name: 'Machine', file: 'system_info', add_to_files: true,
      values: [{ key: 'gpu', name: 'GPU', find: { type: 'value_of', name: 'GPU' }, which: 'first', fallback: null }],
    } as never);
    render(DataFields);
    const row = (await screen.findByText('{gpu}')).closest('tr')!;
    expect(within(row).getByText('after “GPU”')).toBeTruthy();
    await fireEvent.click(screen.getByRole('tab', { name: /Files/ }));
    expect(await screen.findByText('system_info')).toBeTruthy();
    expect(screen.getByText(/Added to the window’s files/)).toBeTruthy();
  });
});
