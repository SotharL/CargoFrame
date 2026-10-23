import { describe, expect, it } from 'vitest';
import { go, parseTarget, restore, route } from './route.svelte';

describe('settings route', () => {
  it('reads a page and an item from the backend target', () => {
    expect(parseTarget('layouts/bug')).toEqual({ page: 'layouts', arg: 'bug' });
    expect(parseTarget('rules')).toEqual({ page: 'rules', arg: null });
    expect(parseTarget('nope/x')).toBeNull();
    expect(parseTarget(null)).toBeNull();
  });

  it('starts on the page in the address, ahead of the remembered one', () => {
    go('safe');
    restore('?page=layouts/bug');
    expect(route).toEqual({ page: 'layouts', arg: 'bug' });
    // Opening from an address does not overwrite what the person last chose themselves.
    restore('');
    expect(route.page).toBe('safe');
  });
});
