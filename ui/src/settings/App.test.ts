import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import App from './App.svelte';
import { go } from './route.svelte';
import { setApiForTests } from '../lib/ipc';
import { memoryApi } from '../lib/memoryApi';

beforeEach(() => {
  setApiForTests(memoryApi());
  go('connections');
});
afterEach(() => {
  cleanup();
  setApiForTests(undefined);
});

describe('Settings sidebar', () => {
  it('groups the pages by what they are about', () => {
    render(App);
    const group = (name: string) =>
      [...screen.getByRole('group', { name }).querySelectorAll('button')].map((b) => b.textContent?.replace(/\d+$/, ''));
    expect(group('Get started')).toEqual(['Connections']);
    expect(group('Captures')).toEqual(['Layouts', 'Shortcuts', 'Data parsing', 'Recording']);
    expect(group('CargoFrame')).toEqual(['Preferences', 'Files & backup', 'System']);
    // each with its icon, and its name on hover (for a sidebar folded to its icons)
    const first = screen.getByRole('button', { name: 'Connections' });
    expect(first.querySelector('svg.icon')).toBeTruthy();
    expect(first.getAttribute('title')).toBe('Connections');
  });

  it('moves through every page with the arrow keys, across the groups', async () => {
    render(App);
    const first = screen.getByRole('button', { name: 'Connections' });
    await fireEvent.keyDown(first, { key: 'ArrowDown' });
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('Layouts');
    await fireEvent.keyDown(screen.getByRole('button', { name: 'Layouts' }), { key: 'ArrowUp' });
    await fireEvent.keyDown(screen.getByRole('button', { name: 'Connections' }), { key: 'ArrowUp' });
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('System');
  });
});

describe('The tour of CargoFrame', () => {
  it('starts when the program asks for it (a new install or an update), and can be skipped', async () => {
    window.history.replaceState(null, '', '/settings.html?page=connections&tour=basics');
    try {
      render(App);
      const say = await screen.findByRole('dialog', { name: 'Tour of CargoFrame' });
      expect(say.textContent).toContain('Connections');
      await fireEvent.click(screen.getByRole('button', { name: 'Skip the tour' }));
      expect(screen.queryByRole('dialog', { name: 'Tour of CargoFrame' })).toBeNull();
    } finally {
      window.history.replaceState(null, '', '/');
    }
  });
});

describe('Settings search', () => {
  const box = () => screen.getByRole('textbox', { name: 'Search settings' }) as HTMLInputElement;

  it('sits above the pages, lists only what has the words, and clears with its ×', async () => {
    render(App);
    const nav = screen.getByRole('navigation', { name: 'Settings sections' });
    const order = [...nav.querySelectorAll('input, button')].map((e) => (e as HTMLElement).getAttribute('aria-label') ?? e.textContent);
    expect(order.indexOf('Search settings')).toBeLessThan(order.findIndex((t) => t?.startsWith('Connections')));
    expect(screen.queryByRole('button', { name: 'Clear the search' })).toBeNull();

    await fireEvent.input(box(), { target: { value: 'DARK theme' } });
    // the groups give way to what was found, by page
    expect(screen.queryByRole('group', { name: 'Captures' })).toBeNull();
    expect(screen.getByRole('button', { name: 'Preferences' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Appearance' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Layouts' })).toBeNull();

    await fireEvent.input(box(), { target: { value: 'zzzz' } });
    expect(screen.getByText('No setting has these words.')).toBeTruthy();

    await fireEvent.click(screen.getByRole('button', { name: 'Clear the search' }));
    expect(box().value).toBe('');
    expect(screen.getByRole('group', { name: 'Captures' })).toBeTruthy();
    expect(document.activeElement).toBe(box());
  });

  it('greys out the sections of the page shown that do not have the words', async () => {
    go('preferences');
    render(App);
    await screen.findByRole('heading', { name: 'Appearance' });
    await fireEvent.input(box(), { target: { value: 'theme' } });
    const section = (title: string) => screen.getByRole('heading', { name: title, level: 2 }).closest('section')!;
    expect(section('Appearance').classList.contains('search-dim')).toBe(false);
    expect(section('Startup').classList.contains('search-dim')).toBe(true);
    // Escape clears, and the page comes back
    await fireEvent.keyDown(box(), { key: 'Escape' });
    expect(box().value).toBe('');
    expect(section('Startup').classList.contains('search-dim')).toBe(false);
  });

  it('goes to what was found and lights it up; Enter goes to the first one', async () => {
    render(App);
    await fireEvent.input(box(), { target: { value: 'follow window' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Follow a window that is moved' }));
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('Recording');
    await new Promise((r) => setTimeout(r, 300));
    expect(document.querySelector('.search-found')?.getAttribute('data-tour')).toBe('video.follow-window');

    await fireEvent.input(box(), { target: { value: 'backup' } });
    await fireEvent.keyDown(box(), { key: 'Enter' });
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('Files & backup');
  });

  it('is found with Ctrl+F', async () => {
    render(App);
    await fireEvent.keyDown(window, { key: 'f', ctrlKey: true });
    expect(document.activeElement).toBe(box());
  });
});
