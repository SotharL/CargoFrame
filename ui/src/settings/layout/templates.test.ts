import { describe, expect, it } from 'vitest';
import { offersFor } from './templates';
import type { ConnectionView, LayoutTemplate } from '../../lib/types';

const templates: LayoutTemplate[] = [
  { key: 'local-capture', name: 'Save a capture locally', description: '', service: null },
  { key: 'jira-bug', name: 'Jira bug report', description: '', service: 'jira' },
  { key: 'jira-comment', name: 'Jira comment', description: '', service: 'jira' },
  { key: 'reddit-post', name: 'Reddit post', description: '', service: 'reddit' },
];

const conn = (id: string, name: string, kind: ConnectionView['kind'], state: ConnectionView['state']): ConnectionView => ({
  id, name, kind, kind_label: kind === 'jira' ? 'Jira' : 'Reddit', enabled: state !== 'disabled', state,
  secrets: [], describe: { summary: '', fields: [], lists: [] },
});

describe('offersFor', () => {
  it('offers only the local template when there is no connection', () => {
    const groups = offersFor(templates, []);
    expect(groups.map((g) => g.state)).toEqual(['local']);
    expect(groups[0]!.offers.map((o) => o.key)).toEqual(['local-capture']);
    expect(groups[0]!.offers[0]!.connectionId).toBeNull();
  });

  it('puts signed-in connections first, then the local one, then those that cannot send yet', () => {
    const groups = offersFor(templates, [
      conn('r', 'Reddit', 'reddit', 'disconnected'),
      conn('z', 'Zed Jira', 'jira', 'connected'),
      conn('a', 'Acme Jira', 'jira', 'connected'),
    ]);
    expect(groups.map((g) => g.title)).toEqual(['Acme Jira', 'Zed Jira', 'No connection', 'Reddit']);
    expect(groups.map((g) => g.state)).toEqual(['ready', 'ready', 'local', 'needs_sign_in']);
    expect(groups[3]!.detail).toBe('Reddit, not signed in yet');
  });

  it('builds each offer for its own connection, one per template of that kind', () => {
    const groups = offersFor(templates, [conn('work', 'Work', 'jira', 'connected')]);
    const jira = groups.find((g) => g.id === 'work')!;
    expect(jira.offers.map((o) => [o.key, o.connectionId])).toEqual([
      ['jira-bug', 'work'],
      ['jira-comment', 'work'],
    ]);
  });

  it('says why a connection is not ready, and skips kinds with no template', () => {
    const groups = offersFor(templates, [
      conn('e', 'Old', 'jira', 'expired'),
      conn('d', 'Off', 'reddit', 'disabled'),
      conn('g', 'Google', 'google', 'connected'),
    ]);
    expect(groups.map((g) => g.id)).toEqual(['', 'd', 'e']);
    expect(groups.find((g) => g.id === 'e')!.detail).toBe('Jira, sign-in expired');
    expect(groups.find((g) => g.id === 'd')!.detail).toBe('Reddit, turned off');
  });
});
