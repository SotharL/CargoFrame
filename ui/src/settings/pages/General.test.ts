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

describe('Accent colour', () => {
  afterEach(() => {
    document.documentElement.style.removeProperty('--accent');
    document.documentElement.style.removeProperty('--on-accent');
  });

  it('sits under Theme in Appearance and starts on the design system blue', async () => {
    render(General);
    const hex = (await screen.findByRole('textbox', { name: /Accent colour as a code/ })) as HTMLInputElement;
    expect(hex.value).toBe('#4f8ee0');
    expect((screen.getByRole('button', { name: 'Reset to blue' }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('a suggested colour colours the window at once and is saved', async () => {
    render(General);
    await screen.findByRole('textbox', { name: /Accent colour as a code/ });
    await fireEvent.click(screen.getByRole('button', { name: 'Orange' }));
    expect(document.documentElement.style.getPropertyValue('--accent')).toBe('#e8590c');
    await waitFor(async () => expect((await api.getGeneral()).accent).toBe('#e8590c'));
    expect(screen.getByRole('button', { name: 'Orange' }).getAttribute('aria-pressed')).toBe('true');
  });

  it('a typed code is saved only when it is a colour, and Reset goes back to the blue', async () => {
    render(General);
    const hex = (await screen.findByRole('textbox', { name: /Accent colour as a code/ })) as HTMLInputElement;
    await fireEvent.input(hex, { target: { value: '#12' } });
    expect(screen.getByRole('alert').textContent).toContain('Type a colour like #4f8ee0');
    await fireEvent.change(hex);
    expect((await api.getGeneral()).accent).toBe('#4f8ee0');

    await fireEvent.input(hex, { target: { value: '#ABC' } });
    await fireEvent.change(hex);
    await waitFor(async () => expect((await api.getGeneral()).accent).toBe('#aabbcc'));

    await fireEvent.click(screen.getByRole('button', { name: 'Reset to blue' }));
    await waitFor(async () => expect((await api.getGeneral()).accent).toBe('#4f8ee0'));
    expect(document.documentElement.style.getPropertyValue('--accent')).toBe('');
  });

  it('says so when a colour is hard to see on one of the themes', async () => {
    render(General);
    const hex = (await screen.findByRole('textbox', { name: /Accent colour as a code/ })) as HTMLInputElement;
    await fireEvent.input(hex, { target: { value: '#fff3a0' } });
    expect(screen.getByText('Hard to see on the light theme')).toBeTruthy();
    await fireEvent.input(hex, { target: { value: '#1d3557' } });
    expect(screen.getByText('Hard to see on the dark theme')).toBeTruthy();
  });

  it('keeps the other settings when the accent is saved', async () => {
    api = memoryApi({ general: { memory_mode: 'always_warm', theme: 'dark' } });
    setApiForTests(api);
    render(General);
    await screen.findByRole('textbox', { name: /Accent colour as a code/ });
    await fireEvent.click(screen.getByRole('button', { name: 'Teal' }));
    await waitFor(async () => {
      const g = await api.getGeneral();
      expect(g).toMatchObject({ accent: '#1f9d8f', memory_mode: 'always_warm', theme: 'dark' });
    });
  });
});
