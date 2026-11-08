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
  it('holds how CargoFrame looks, starts, tells you things, keeps windows and handles a failed share', async () => {
    render(General);
    await screen.findByRole('heading', { name: 'When sharing fails' });
    const titles = [...document.querySelectorAll('section > h2')].map((h) => h.textContent);
    expect(titles).toEqual(['Appearance', 'Startup', 'Notifications', 'When sharing fails', 'Windows kept ready']);
  });

  it('saves the memory setting and says what it does', async () => {
    render(General);
    const box = await screen.findByRole('combobox', { name: 'Keep windows ready' });
    await fireEvent.change(box, { target: { value: 'low_memory' } });
    await waitFor(async () => expect((await api.getGeneral()).memory_mode).toBe('low_memory'));
    expect(screen.getByText(/Frees the memory as soon as a window closes/)).toBeTruthy();
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
