import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import App from './App.svelte';
import { go } from './route.svelte';
import { memoryApi, setApiForTests } from '../lib/ipc';

beforeEach(() => {
  setApiForTests(memoryApi());
  go('setup');
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
    expect(group('Get started')).toEqual(['Setup']);
    expect(group('Captures')).toEqual(['Layouts', 'Triggers', 'Data & fields', 'Recording']);
    expect(group('CargoFrame')).toEqual(['General', 'Your data', 'About']);
  });

  it('moves through every page with the arrow keys, across the groups', async () => {
    render(App);
    const setup = screen.getByRole('button', { name: 'Setup' });
    await fireEvent.keyDown(setup, { key: 'ArrowDown' });
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('Layouts');
    await fireEvent.keyDown(screen.getByRole('button', { name: 'Layouts' }), { key: 'ArrowUp' });
    await fireEvent.keyDown(screen.getByRole('button', { name: 'Setup' }), { key: 'ArrowUp' });
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('About');
  });
});

describe('Settings search', () => {
  const box = () => screen.getByRole('textbox', { name: 'Search settings' }) as HTMLInputElement;

  it('sits above Setup, lists only what has the words, and clears with its ×', async () => {
    render(App);
    const nav = screen.getByRole('navigation', { name: 'Settings sections' });
    const order = [...nav.querySelectorAll('input, button')].map((e) => (e as HTMLElement).getAttribute('aria-label') ?? e.textContent);
    expect(order.indexOf('Search settings')).toBeLessThan(order.findIndex((t) => t?.startsWith('Setup')));
    expect(screen.queryByRole('button', { name: 'Clear the search' })).toBeNull();

    await fireEvent.input(box(), { target: { value: 'DARK theme' } });
    // the groups give way to what was found, by page
    expect(screen.queryByRole('group', { name: 'Captures' })).toBeNull();
    expect(screen.getByRole('button', { name: 'General' })).toBeTruthy();
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
    go('general');
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
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('Your data');
  });

  it('is found with Ctrl+F', async () => {
    render(App);
    await fireEvent.keyDown(window, { key: 'f', ctrlKey: true });
    expect(document.activeElement).toBe(box());
  });
});
