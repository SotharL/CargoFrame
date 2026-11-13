// The whole CargoFrame window, drawn from the sample captures: it must get past "Loading…" for
// every kind of layout. A bound value missing at the start once stopped it for every layout
// with a dropdown, and no test of a single block could see it.
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import App from './App.svelte';
import { memoryApi, setApiForTests } from '../lib/ipc';

// jsdom draws no pictures: the screenshot block says so instead, and the window still opens
globalThis.createImageBitmap ??= (() => Promise.reject(new Error('no pictures here'))) as typeof createImageBitmap;

afterEach(() => {
  cleanup();
  setApiForTests(undefined);
  history.replaceState(null, '', '/');
});

async function opens(search: string) {
  history.replaceState(null, '', `/capture.html${search}`);
  setApiForTests(memoryApi());
  render(App);
  // the trigger's name heads the window once it has drawn
  expect(await screen.findByRole('heading', { level: 1 })).toBeTruthy();
  expect(screen.queryByText('Loading…')).toBeNull();
  expect(screen.queryByText('CargoFrame could not open this capture')).toBeNull();
}

describe('the CargoFrame window', () => {
  it('opens a screenshot layout with dropdowns', () => opens(''));
  it('opens a layout with a Discord path that goes down to threads', () => opens('?demo=discord'));
  it('opens a recording', () => opens('?media=video'));
  it('opens a layout with both a screenshot and a recording', () => opens('?media=both'));
  it('opens one whose screenshot was taken when the recording stopped', () => opens('?media=both&end=1'));
});

describe('tabs in the window', () => {
  it('shows the blocks of the open tab and keeps the others, only hidden', async () => {
    const text = (id: string, y: number, title: string) => ({ id, type: 'text', rect: { x: 0, y, w: 6, h: 1 }, title, placement: 'above', role: 'plain', default_text: '' });
    const blocks = [
      { id: 'tabs', type: 'tabs', rect: { x: 0, y: 0, w: 6, h: 3 }, tabs: [{ name: 'Details', blocks: ['a'] }, { name: 'More', blocks: ['b'] }] },
      text('a', 0, 'Summary'),
      text('b', 1, 'Build'),
    ];
    await opens(`?blocks=${encodeURIComponent(JSON.stringify(blocks))}`);
    const hidden = (title: string) => screen.getByText(title).closest('.slot')!.classList.contains('hidden');
    expect([hidden('Summary'), hidden('Build')]).toEqual([false, true]);
    await fireEvent.click(screen.getByRole('tab', { name: 'More' }));
    expect([hidden('Summary'), hidden('Build')]).toEqual([true, false]);
  });
});
