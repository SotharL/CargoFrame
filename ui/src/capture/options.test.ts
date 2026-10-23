import { describe, expect, it } from 'vitest';
import { loadDropdowns, withFailure, withOptions } from './options';
import { demoSession } from './demo';
import type { DropdownState } from '../lib/types';

const loading: DropdownState = { status: 'loading', options: [], selected: null, message: null };
const opts = [
  { value: 'a', label: 'A', group: null },
  { value: 'b', label: 'B', group: null },
];

describe('dropdown lists', () => {
  it('picks the first option, or keeps a choice that is still offered', () => {
    expect(withOptions(loading, opts, null).selected).toBe('a');
    expect(withOptions(loading, opts, 'b').selected).toBe('b');
    expect(withOptions(loading, opts, 'gone').selected).toBe('a');
    const none = withOptions(loading, [], null);
    expect(none.selected).toBeNull();
    expect(none.message).toMatch(/no options/);
  });

  it('says to reconnect when the sign-in expired', () => {
    const f = withFailure(loading, 'Sign-in expired.', true);
    expect(f.status).toBe('needs_connection');
    expect(f.message).toContain('Reconnect it in Settings');
    expect(withFailure(loading, 'No network.', false).message).toBe('No network.');
  });

  it('loads every loading dropdown, reports failures, and drops answers for an old capture', async () => {
    const view = await demoSession();
    view.dropdowns['proj'] = { ...loading };
    view.dropdowns['other'] = { ...loading };
    const seen: Record<string, DropdownState> = {};
    const api = {
      fetchOptions: async (_s: number, block: string) => {
        if (block === 'other') throw { message: 'Rejected.', reconnect: true };
        return opts;
      },
    };
    await loadDropdowns(api, view, () => true, (id, next) => {
      seen[id] = next(view.dropdowns[id]!);
    });
    expect(seen['proj']!.status).toBe('ready');
    expect(seen['proj']!.selected).toBe('a');
    expect(seen['other']!.status).toBe('needs_connection');
    expect(seen['other']!.message).toContain('Reconnect');
    // the ready demo dropdown (Claude chats) is not fetched
    expect(seen['chat']).toBeUndefined();

    const stale: string[] = [];
    await loadDropdowns(api, view, () => false, (id) => stale.push(id));
    expect(stale).toEqual([]);
  });
});
