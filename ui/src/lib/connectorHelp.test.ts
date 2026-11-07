import { describe, expect, it } from 'vitest';
import { getLinkFor, type HelpField } from './connectorHelp';
import type { Connection } from './types';

const base = { name: 'x', enabled: true };
const connection = (c: Partial<Connection> & { kind: Connection['kind'] }): Connection => ({ ...base, ...c }) as Connection;

const jira = (auth: 'api_token' | 'pat', site = 'https://acme.atlassian.net') =>
  connection({ kind: 'jira', auth, site_url: site, email: '' });

/** Every field a connection asks for that a service's website hands out. Add a field here when one is added to a form. */
const ASKED: [Connection, HelpField[]][] = [
  [jira('api_token'), ['token']],
  [jira('pat'), ['token']],
  [connection({ kind: 'google', client_id: '', sheets: false, full_drive: false }), ['client_id', 'client_secret']],
  [connection({ kind: 'reddit', client_id: '', redirect_port: 53682 }), ['client_id']],
  [connection({ kind: 'slack' }), ['token']],
  [connection({ kind: 'discord', mode: 'bot', shown_as: '', hooks: [] }), ['token', 'webhook']],
  [connection({ kind: 'teams', client_id: '', tenant: 'organizations' }), ['client_id']],
];

describe('where to get what a connection asks for', () => {
  it('has a page for every field a form asks for', () => {
    for (const [c, fields] of ASKED) {
      for (const f of fields) expect(getLinkFor(c, f), `${c.kind} ${f}`).not.toBeNull();
    }
  });

  it('only ever opens https pages', () => {
    for (const [c, fields] of ASKED) {
      for (const f of fields) expect(getLinkFor(c, f)!.url).toMatch(/^https:\/\//);
    }
  });

  it('sends a Jira Cloud user to Atlassian and a Server user to their own profile page', () => {
    expect(getLinkFor(jira('api_token'), 'token')!.url).toBe('https://id.atlassian.com/manage-profile/security/api-tokens');
    expect(getLinkFor(jira('pat', 'https://jira.corp.example/'), 'token')!.url).toBe('https://jira.corp.example/secure/ViewProfile.jspa');
  });

  it('has no page for a Server address that is not written yet, or is not https', () => {
    expect(getLinkFor(jira('pat', 'https://'), 'token')).toBeNull();
    expect(getLinkFor(jira('pat', 'http://jira.corp.example'), 'token')).toBeNull();
    expect(getLinkFor(jira('pat', ''), 'token')).toBeNull();
  });

  it('has none for a custom API or a field that is not handed out', () => {
    const custom = connection({ kind: 'custom', base_url: '', auth: { type: 'none' } } as never);
    expect(getLinkFor(custom, 'token')).toBeNull();
    expect(getLinkFor(connection({ kind: 'slack' }), 'client_id')).toBeNull();
  });
});
