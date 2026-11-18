import { describe, expect, it } from 'vitest';
import { PAGES } from './nav';
import { go, parseTarget, restore, route } from './route.svelte';

describe('settings route', () => {
  it('reads a page and an item from the backend target', () => {
    expect(parseTarget('layouts/bug')).toEqual({ page: 'layouts', arg: 'bug' });
    expect(parseTarget('rules')).toEqual({ page: 'rules', arg: null });
    expect(parseTarget('nope/x')).toBeNull();
    expect(parseTarget(null)).toBeNull();
  });

  it('starts on the page in the address, ahead of the remembered one', () => {
    go('rules');
    restore('?page=layouts/bug');
    expect(route).toEqual({ page: 'layouts', arg: 'bug' });
    // Opening from an address does not overwrite what the person last chose themselves.
    restore('');
    expect(route.page).toBe('rules');
  });

  it('opens Files & backup for the old Safe settings address and the old remembered page', () => {
    expect(parseTarget('safe')).toEqual({ page: 'files', arg: null });
    localStorage.setItem('cargoframe.settings.page', 'safe');
    restore('');
    expect(route.page).toBe('files');
  });

  it('follows the order a new person sets things up in, with the program itself last', () => {
    expect(PAGES.map((p) => p.id)).toEqual(['connections', 'layouts', 'triggers', 'rules', 'video', 'preferences', 'files', 'system']);
    expect(PAGES.map((p) => p.group)).toEqual(['start', 'capture', 'capture', 'capture', 'capture', 'app', 'app', 'app']);
    expect(PAGES.some((p) => (p.id as string) === 'safe')).toBe(false);
  });

  it('keeps old addresses working', () => {
    expect(parseTarget('recording')).toEqual({ page: 'video', arg: null });
    expect(parseTarget('updates')).toEqual({ page: 'system', arg: null });
    // the pages of 0.6 and before
    expect(parseTarget('setup')).toEqual({ page: 'system', arg: null });
    expect(parseTarget('setup/connections')).toEqual({ page: 'connections', arg: null });
    expect(parseTarget('general')).toEqual({ page: 'preferences', arg: null });
    expect(parseTarget('data')).toEqual({ page: 'files', arg: null });
    expect(parseTarget('about')).toEqual({ page: 'system', arg: null });
  });

  it('gives every page its own icon', () => {
    expect(new Set(PAGES.map((p) => p.icon)).size).toBe(PAGES.length);
  });
});
