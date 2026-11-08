import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import About from './About.svelte';
import { memoryApi, setApiForTests, type Api } from '../../lib/ipc';

let api: Api;
beforeEach(() => {
  api = memoryApi();
  setApiForTests(api);
});
afterEach(() => {
  cleanup();
  setApiForTests(undefined);
});

describe('About', () => {
  it('has the version and updates first, then speed and memory', async () => {
    render(About);
    await screen.findByRole('heading', { name: 'Updates' });
    const titles = [...document.querySelectorAll('section > h2')].map((h) => h.textContent);
    expect(titles).toEqual(['Updates', 'Speed and memory']);
  });

  it('saves the daily update check as a General setting', async () => {
    render(About);
    const box = (await screen.findByRole('switch', { name: 'Look for a newer version once a day' })) as HTMLInputElement;
    await fireEvent.click(box);
    await waitFor(async () => expect((await api.getGeneral()).check_updates).toBe(true));
  });

  it('shows what each window holds now', async () => {
    render(About);
    expect(await screen.findByText('Capture window')).toBeTruthy();
    expect(screen.getByText('Hidden, kept ready (holds memory)')).toBeTruthy();
    expect(screen.getByText('Open now')).toBeTruthy();
  });

  it('Refresh visibly refreshes, then says when', async () => {
    render(About);
    await screen.findByText('Capture window');
    await fireEvent.click(screen.getByRole('button', { name: 'Refresh' }));
    expect(screen.getByRole('button', { name: 'Refreshing…' })).toBeTruthy();
    expect(screen.getByText('Reading the current state…')).toBeTruthy();
    await waitFor(() => expect(screen.getByText(/^Refreshed at /)).toBeTruthy(), { timeout: 3000 });
  });
});
