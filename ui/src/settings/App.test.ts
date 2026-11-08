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
    expect(group('Captures')).toEqual(['Layouts', 'Triggers', 'Data from files', 'Extra fields', 'Recording']);
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
