import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import About from './System.svelte';
import { setApiForTests, type Api } from '../../lib/ipc';
import { memoryApi } from '../../lib/memoryApi';

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
  it('has the apps on this computer, the updates, the memory, debug mode, then the speed', async () => {
    render(About);
    await screen.findByRole('heading', { name: 'Windows kept ready' });
    const titles = [...document.querySelectorAll('section h2')].map((h) => h.textContent);
    expect(titles.filter((t) => t !== 'Install what is missing')).toEqual(['This computer', 'Updates', 'Windows kept ready', 'Debug mode', 'Speed and memory']);
  });

  it('sets the bug report up with one button once debug mode is on', async () => {
    render(About);
    await fireEvent.click(await screen.findByRole('switch', { name: 'Describe CargoFrame in a capture of its own windows' }));
    await fireEvent.click(await screen.findByRole('button', { name: 'Set up the bug report' }));
    expect(await screen.findByText('The bug report is set up')).toBeTruthy();
    expect(screen.getByText(/press Ctrl\+Alt\+Shift\+F12/)).toBeTruthy();
  });

  it('checks this PC and lists each step with how it went', async () => {
    setApiForTests(memoryApi({ general: { debug: true } }));
    render(About);
    await fireEvent.click(await screen.findByRole('button', { name: 'Check this PC' }));
    const steps = await screen.findByRole('list', { name: 'What was checked' });
    expect(steps.textContent).toContain('A 3-second recording');
    expect(steps.querySelector('li.warn')?.textContent).toContain('Another copy starts at sign-in');
    expect(screen.getByRole('button', { name: 'Check this PC again' })).toBeTruthy();
    expect(screen.getByText('Kept for the next bug report')).toBeTruthy();
  });

  it('offers Check this PC even when the bug report setup cannot be read', async () => {
    const api = memoryApi({ general: { debug: true } });
    api.debugSetupStatus = async () => { throw new Error('no'); };
    setApiForTests(api);
    render(About);
    expect(await screen.findByRole('button', { name: 'Check this PC' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Set up the bug report' })).toBeNull();
  });

  it('asks for a GitHub connection first when there is none', async () => {
    setApiForTests(memoryApi({ general: { debug: true }, github: [] }));
    render(About);
    expect(await screen.findByRole('button', { name: 'Go to Connections' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Set up the bug report' })).toBeNull();
  });

  it('turns debug mode on and off, and keeps it', async () => {
    render(About);
    const toggle = await screen.findByRole('switch', { name: 'Describe CargoFrame in a capture of its own windows' });
    expect((toggle as HTMLInputElement).checked).toBe(false);
    await fireEvent.click(toggle);
    await waitFor(async () => expect((await api.getGeneral()).debug).toBe(true));
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

describe('System under an organisation policy', () => {
  it('locks the update check and debug mode the policy decides', async () => {
    setApiForTests(memoryApi({ policy: { services: [], check_updates: true, debug_mode: false, own_connections: null } }));
    render(About);
    const updates = (await screen.findByRole('switch', { name: 'Look for a newer version once a day' })) as HTMLInputElement;
    expect(updates.disabled).toBe(true);
    const debug = screen.getByRole('switch', { name: 'Describe CargoFrame in a capture of its own windows' }) as HTMLInputElement;
    expect(debug.disabled).toBe(true);
    expect(screen.getAllByText('Set by your organisation')).toHaveLength(2);
  });
});
