import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import DeeperStep from './DeeperStep.svelte';
import { memoryApi, setApiForTests } from '../../lib/ipc';
import { deeperKey, remember } from '../../lib/recent';
import type { DropdownOption } from '../../lib/types';

const post = (value: string, label: string, group: string | null = null): DropdownOption => ({ value, label, group });
const POSTS: Record<string, DropdownOption[]> = {
  'where:C1': [post('p1', 'Crash on save', 'Active'), post('p2', 'Slow start', 'Active'), post('p3', 'Crash in export', 'Older')],
  'where:C2': [post('p9', 'Ideas', 'Active')],
};

function open(parent: string, extra: Record<string, unknown> = {}, list: (l: string) => DropdownOption[] | Promise<DropdownOption[]> = (l) => POSTS[l] ?? []) {
  const asked: string[] = [];
  setApiForTests(memoryApi({ options: (_c, l) => (asked.push(l), list(l)) }));
  const view = render(DeeperStep, { sessionId: 1, blockId: 'where', layoutId: 'lay', parent, ...extra });
  return { asked, view };
}
const box = () => screen.getByRole('combobox', { name: 'Thread or post' }) as HTMLInputElement;
const listed = () => within(screen.getByRole('listbox')).getAllByRole('option').map((o) => o.textContent?.replace(/\s+/g, ' ').trim());

beforeEach(() => localStorage.clear());
afterEach(() => {
  cleanup();
  setApiForTests(undefined);
});

describe('DeeperStep', () => {
  it('asks for what is inside the channel, and starts on a new message', async () => {
    const { asked } = open('C1');
    await waitFor(() => expect(box().disabled).toBe(false));
    expect(asked).toEqual(['where:C1']);
    expect(box().value).toBe('New message or post');
    await fireEvent.focus(box());
    // "new" first, then the posts as the service ordered them
    expect(listed()).toEqual(['New message or post', 'Crash on save', 'Slow start', 'Crash in export']);
  });

  it('picks a post from the list, and goes back to new', async () => {
    open('C1');
    await waitFor(() => expect(box().disabled).toBe(false));
    await fireEvent.focus(box());
    await fireEvent.pointerDown(screen.getByRole('option', { name: /Slow start/ }));
    expect(box().value).toBe('Slow start');
    await fireEvent.focus(box());
    await fireEvent.pointerDown(screen.getByRole('option', { name: /New message or post/ }));
    expect(box().value).toBe('New message or post');
  });

  it('starts on the post used last in this channel, and on new in another', async () => {
    remember('lay', deeperKey('where', 'C1'), { value: 'p2', label: 'Slow start' });
    open('C1');
    await waitFor(() => expect(box().disabled).toBe(false));
    expect(box().value).toBe('Slow start');
    cleanup();
    open('C2');
    await waitFor(() => expect(box().disabled).toBe(false));
    expect(box().value).toBe('New message or post');
  });

  it('does not start on a remembered post when the block does not remember', async () => {
    remember('lay', deeperKey('where', 'C1'), { value: 'p2', label: 'Slow start' });
    open('C1', { remembers: false });
    await waitFor(() => expect(box().disabled).toBe(false));
    expect(box().value).toBe('New message or post');
  });

  it('narrows to the posts named like the title, and can show them all', async () => {
    open('C1', { narrowText: 'crash' });
    await waitFor(() => expect(box().disabled).toBe(false));
    expect(screen.getByText(/Showing the 2 with “crash” in the name/)).toBeTruthy();
    await fireEvent.focus(box());
    expect(listed()).toEqual(['New message or post', 'Crash on save', 'Crash in export']);
    await fireEvent.click(screen.getByRole('button', { name: 'Show all' }));
    await fireEvent.focus(box());
    expect(listed()).toContain('Slow start');
  });

  it('says so when no post is named like the title, and shows them all', async () => {
    open('C1', { narrowText: 'nothing like it' });
    await waitFor(() => expect(box().disabled).toBe(false));
    expect(screen.getByText(/None has “nothing like it” in the name, so all are shown/)).toBeTruthy();
    await fireEvent.focus(box());
    expect(listed()).toHaveLength(4);
  });

  it('keeps long lists searchable by name', async () => {
    const many = Array.from({ length: 40 }, (_, i) => post(`t${i}`, `Ticket ${i}`, 'Older'));
    open('C1', {}, () => many);
    await waitFor(() => expect(box().placeholder).toBe('Search 40 or paste a link…'));
    await fireEvent.focus(box());
    await fireEvent.input(box(), { target: { value: 'ticket 3' } });
    // Ticket 3, and Ticket 30 to 39
    expect(listed()).toHaveLength(11);
  });

  it('with a webhook nothing is listed: the posts made before are offered, and a link can be pasted', async () => {
    remember('lay', deeperKey('where', 'C1'), { value: 'p7', label: 'Build 12 is red' });
    open('C1', { remembers: false }, () => []);
    await waitFor(() => expect(box().disabled).toBe(false));
    await fireEvent.focus(box());
    expect(listed()).toEqual(['New message or post', 'Build 12 is red']);
    await fireEvent.input(box(), { target: { value: 'https://discord.com/channels/1/2/3' } });
    expect(box().value).toBe('https://discord.com/channels/1/2/3');
    expect(screen.queryByText(/is not in the list and is not a link/)).toBeNull();
  });

  it('explains an empty list instead of showing nothing', async () => {
    open('C1', {}, () => []);
    await waitFor(() => expect(screen.getByText(/No threads are listed here/)).toBeTruthy());
  });

  it('still offers a new post when the list cannot be read, and says why', async () => {
    open('C1', {}, () => Promise.reject({ message: 'The bot may not read this channel', reconnect: false }));
    await waitFor(() => expect(screen.getByText('The bot may not read this channel')).toBeTruthy());
    expect(box().disabled).toBe(false);
    expect(box().value).toBe('New message or post');
  });

  it('warns about half a word that is neither a post nor a link', async () => {
    open('C1');
    await waitFor(() => expect(box().disabled).toBe(false));
    await fireEvent.input(box(), { target: { value: 'sl' } });
    expect(screen.getByText(/“sl” is not in the list and is not a link/)).toBeTruthy();
  });

  it('starts over for another channel, and ignores a late answer for the old one', async () => {
    let finish: (o: DropdownOption[]) => void = () => {};
    const { view } = open('C1', {}, (l) => (l === 'where:C1' ? new Promise<DropdownOption[]>((r) => (finish = r)) : (POSTS[l] ?? [])));
    await view.rerender({ sessionId: 1, blockId: 'where', layoutId: 'lay', parent: 'C2' });
    await waitFor(() => expect(box().disabled).toBe(false));
    finish(POSTS['where:C1']!);
    await fireEvent.focus(box());
    expect(listed()).toEqual(['New message or post', 'Ideas']);
  });
});
