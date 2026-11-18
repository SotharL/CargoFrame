import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import About from './System.svelte';
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

describe('System', () => {
  it('has the apps on this computer, the updates, the memory, then the speed', async () => {
    render(About);
    await screen.findByRole('heading', { name: 'Windows kept ready' });
    const titles = [...document.querySelectorAll('section h2')].map((h) => h.textContent);
    expect(titles.filter((t) => t !== 'Install what is missing')).toEqual(['This computer', 'Updates', 'Windows kept ready', 'Speed and memory']);
  });

  it('saves the memory setting and says what it does', async () => {
    render(About);
    const box = await screen.findByRole('combobox', { name: 'Keep windows ready' });
    await fireEvent.change(box, { target: { value: 'low_memory' } });
    await waitFor(async () => expect((await api.getGeneral()).memory_mode).toBe('low_memory'));
    expect(screen.getByText(/The memory is freed as soon as a window closes/)).toBeTruthy();
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
