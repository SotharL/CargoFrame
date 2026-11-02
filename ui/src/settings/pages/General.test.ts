import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import General from './General.svelte';
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

describe('General settings', () => {
  it('lists the sections in the order the person asked for, Updates last', async () => {
    render(General);
    // Safety and Export and import load their own data, so wait for the last of them.
    await screen.findByRole('heading', { name: 'Safety' });
    await screen.findByRole('heading', { name: 'Export and import' });
    const titles = [...document.querySelectorAll('section > h2')].map((h) => h.textContent);
    expect(titles).toEqual(['Appearance', 'Startup', 'Safety', 'Export and import', 'Files', 'Performance', 'Updates']);
  });

  it('shows what the memory setting is doing to each window', async () => {
    render(General);
    expect(await screen.findByText('Capture window')).toBeTruthy();
    expect(screen.getByText('Hidden, kept ready (holds memory)')).toBeTruthy();
    expect(screen.getByText('Open now')).toBeTruthy();
  });

  it('Refresh visibly refreshes, then says when, and picks up a changed setting', async () => {
    render(General);
    await screen.findByText('Capture window');
    await fireEvent.change(screen.getByRole('combobox', { name: 'Keep windows ready' }), { target: { value: 'low_memory' } });
    // Changing the setting refreshes the numbers too.
    await waitFor(() => expect(screen.getByText('Closed (no memory held)')).toBeTruthy(), { timeout: 3000 });
    await waitFor(() => expect(screen.getByRole('button', { name: 'Refresh' })).toBeTruthy(), { timeout: 3000 });

    await fireEvent.click(screen.getByRole('button', { name: 'Refresh' }));
    expect(screen.getByRole('button', { name: 'Refreshing…' })).toBeTruthy();
    expect(screen.getByText('Reading the current state…')).toBeTruthy();
    await waitFor(() => expect(screen.getByText(/^Refreshed at /)).toBeTruthy(), { timeout: 3000 });
    expect(screen.getByRole('button', { name: 'Refresh' })).toBeTruthy();
  });
});
