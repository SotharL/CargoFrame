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

  it('opens General for the old Safe settings address and the old remembered page', () => {
    expect(parseTarget('safe')).toEqual({ page: 'general', arg: null });
    localStorage.setItem('cargoframe.settings.page', 'safe');
    restore('');
    expect(route.page).toBe('general');
  });

  it('puts General first, then Setup (which holds Connections) and Triggers', () => {
    expect(PAGES.map((p) => p.id).slice(0, 3)).toEqual(['general', 'setup', 'triggers']);
    expect(PAGES.some((p) => (p.id as string) === 'safe')).toBe(false);
  });
});
