import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/svelte';
import DropdownBlock from './DropdownBlock.svelte';
import type { Block, DropdownState } from '../../lib/types';

type Dropdown = Extract<Block, { type: 'dropdown' }>;
const block: Dropdown = {
  id: 'labels', type: 'dropdown', rect: { x: 0, y: 0, w: 4, h: 2 }, title: 'Label',
  source: { type: 'web_api', connection_id: 'work', options_id: 'labels' },
};
const info = (n: number): DropdownState => ({
  status: 'ready',
  options: Array.from({ length: n }, (_, i) => ({ value: `label-${i}`, label: `Label ${i}`, group: null })),
  selected: null,
  message: null,
});

describe('DropdownBlock', () => {
  afterEach(cleanup);

  it('a short list with nothing used yet stays a plain drop down', () => {
    render(DropdownBlock, { block, info: info(5), value: null });
    expect(screen.getByRole('combobox', { name: 'Label' }).tagName).toBe('SELECT');
  });

  it('a long list can be searched', async () => {
    render(DropdownBlock, { block, info: info(40), value: null });
    const box = screen.getByRole('combobox', { name: 'Label' });
    expect(box.tagName).toBe('INPUT');
    await fireEvent.focus(box);
    await fireEvent.input(box, { target: { value: 'label 3' } });
    // Label 3, and Label 30 to 39.
    expect(screen.getAllByRole('option')).toHaveLength(11);
    await fireEvent.pointerDown(screen.getByRole('option', { name: 'Label 34' }));
    expect((box as HTMLInputElement).value).toBe('Label 34');
  });
});

const channel: Dropdown = { ...block, id: 'channel', title: 'Channel', rect: { x: 0, y: 0, w: 5, h: 1 } };
const o = (value: string, label: string, server: string, category: string | null = null) => ({
  value, label, group: null, server, category,
});
const discord = (options: ReturnType<typeof o>[]): DropdownState => ({ status: 'ready', options, selected: null, message: null });
const twoServers = discord([
  o('1', '#general', 'Acme'),
  o('2', '#bugs', 'Acme', 'Support'),
  o('3', '#ideas', 'Acme', 'Product'),
  o('4', '#lounge', 'Friends'),
  o('5', '#games', 'Friends'),
]);
const select = (name: string) => screen.queryByRole('combobox', { name }) as HTMLSelectElement | null;
const listed = () => within(screen.getByRole('listbox')).getAllByRole('option');
const shown = () => (screen.getByRole('combobox', { name: 'Channel' }) as HTMLInputElement).value;

describe('DropdownBlock with servers and categories', () => {
  afterEach(cleanup);

  it('asks which server, then which category, then which channel', async () => {
    render(DropdownBlock, { block: channel, info: twoServers, value: '1' });
    expect(select('Server')!.value).toBe('Acme');
    expect(select('Category')!.value).toBe('');
    expect(shown()).toBe('#general');

    // Another server: its channels only, and the first one is chosen.
    await fireEvent.change(select('Server')!, { target: { value: 'Friends' } });
    expect(shown()).toBe('#lounge');
    // Friends has no categories, so that step is gone.
    expect(select('Category')).toBeNull();
    await fireEvent.focus(screen.getByRole('combobox', { name: 'Channel' }));
    expect(listed().map((x) => x.textContent?.trim())).toEqual(['#lounge', '#games']);
  });

  it('narrows to a category and picks its first channel', async () => {
    render(DropdownBlock, { block: channel, info: twoServers, value: '1' });
    await fireEvent.change(select('Category')!, { target: { value: 'Support' } });
    expect(shown()).toBe('#bugs');
  });

  it('skips the server step when there is one server, and the category step when there is one', () => {
    const one = discord([o('1', '#general', 'Acme', 'Only'), o('2', '#bugs', 'Acme', 'Only')]);
    render(DropdownBlock, { block: channel, info: one, value: '1' });
    expect(select('Server')).toBeNull();
    expect(select('Category')).toBeNull();
    expect(shown()).toBe('#general');
  });

  it('finds a channel of another server by typing its name', async () => {
    render(DropdownBlock, { block: channel, info: twoServers, value: '1' });
    const box = screen.getByRole('combobox', { name: 'Channel' });
    await fireEvent.focus(box);
    await fireEvent.input(box, { target: { value: 'games' } });
    await fireEvent.pointerDown(screen.getByRole('option', { name: /#games/ }));
    expect(shown()).toBe('#games');
    // The steps follow the choice.
    expect(select('Server')!.value).toBe('Friends');
  });

  it('lists the recent choices first, under a heading', async () => {
    const recent = [{ value: '4', label: '#lounge' }, { value: '2', label: '#bugs' }];
    render(DropdownBlock, { block: channel, info: twoServers, value: '1', recent });
    await fireEvent.focus(screen.getByRole('combobox', { name: 'Channel' }));
    expect(screen.getByText('Recent')).toBeTruthy();
    const first = listed().slice(0, 2).map((x) => x.textContent?.replace(/\s+/g, ' ').trim());
    expect(first).toEqual(['#lounge Friends', '#bugs Acme · Support']);
  });

  it('says there is nothing to post in when the list is empty', () => {
    render(DropdownBlock, { block: channel, info: discord([]), value: null });
    expect(screen.getByText(/Nothing to post in yet/)).toBeTruthy();
  });
});

describe('an optional list (a thread to reply in)', () => {
  afterEach(cleanup);
  const thread: Dropdown = { ...block, id: 'thread', title: 'Thread', optional: true, narrow_by: 'title' };
  const threads = discord([
    o('t1', 'Crash on save', 'Acme', '#bugs'),
    o('t2', 'Ideas', 'Acme', '#chat'),
    o('t3', 'crash in export', 'Acme', '#bugs'),
  ]);

  it('starts on none, and takes a pasted link', async () => {
    let value: string | null = '';
    const { component } = render(DropdownBlock, { block: thread, info: threads, value, narrowText: '' });
    void component;
    const box = screen.getByRole('combobox', { name: 'Thread' }) as HTMLInputElement;
    expect(box.value).toBe('');
    await fireEvent.input(box, { target: { value: 'https://discord.com/channels/1/2' } });
    expect(box.value).toBe('https://discord.com/channels/1/2');
    void value;
  });

  it('is only a box to paste in when the service has no list', () => {
    render(DropdownBlock, { block: thread, info: discord([]), value: '' });
    const box = screen.getByRole('combobox', { name: 'Thread' }) as HTMLInputElement;
    expect(box.placeholder).toBe('Paste a link (optional)');
    expect(screen.queryByText(/Nothing to post in yet/)).toBeNull();
  });

  it('narrows to the threads named like the title, and can show all', async () => {
    render(DropdownBlock, { block: thread, info: threads, value: '', narrowText: 'crash' });
    expect(screen.getByText(/Showing the 2 with “crash” in the name/)).toBeTruthy();
    const box = screen.getByRole('combobox', { name: 'Thread' });
    await fireEvent.focus(box);
    expect(listed()).toHaveLength(2);
    await fireEvent.click(screen.getByRole('button', { name: 'Show all' }));
    await fireEvent.focus(box);
    expect(listed()).toHaveLength(3);
  });

  it('says so when no thread has the title in its name', () => {
    render(DropdownBlock, { block: thread, info: threads, value: '', narrowText: 'zzz' });
    expect(screen.getByText(/None has “zzz” in the name, so all are shown/)).toBeTruthy();
  });
});
