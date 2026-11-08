// The whole CargoFrame window, drawn from the sample captures: it must get past "Loading…" for
// every kind of layout. A bound value missing at the start once stopped it for every layout
// with a dropdown, and no test of a single block could see it.
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/svelte';
import App from './App.svelte';
import { memoryApi, setApiForTests } from '../lib/ipc';

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
}

describe('the CargoFrame window', () => {
  it('opens a screenshot layout with dropdowns', () => opens(''));
  it('opens a layout with a Discord path that goes down to threads', () => opens('?demo=discord'));
  it('opens a recording', () => opens('?media=video'));
  it('opens a layout with both a screenshot and a recording', () => opens('?media=both'));
  it('opens one whose screenshot was taken when the recording stopped', () => opens('?media=both&end=1'));
});
