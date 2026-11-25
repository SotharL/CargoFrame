import { describe, expect, it } from 'vitest';
import { demoSession } from './demo';
import { initialValues } from './values';
import { needsAsk, unsent } from './needs';
import type { Block, SessionView } from '../lib/types';

type Button = Extract<Block, { type: 'sharing_button' }>;

/** The demo capture with a Jira button that maps a project, an issue type and a note. */
async function jira(): Promise<{ view: SessionView; button: Button }> {
  const view = await demoSession();
  view.layout!.blocks.push({
    id: 'type', type: 'dropdown', rect: { x: 0, y: 20, w: 4, h: 1 }, title: 'Issue type',
    source: { type: 'web_api', connection_id: 'jira', options_id: 'issue_types' },
  });
  view.dropdowns.type = { status: 'ready', options: [], selected: '10001', message: null };
  view.dropdowns.proj!.selected = 'SP';
  const button = view.layout!.blocks.find((b): b is Button => b.type === 'sharing_button')!;
  button.mappings = [
    { field: 'project', source: { kind: 'block', value: 'proj' } },
    { field: 'issuetype', source: { kind: 'block', value: 'type' } },
    { field: 'environment', source: { kind: 'block', value: 'note' } },
  ];
  view.sharing[button.id] = { enabled: true, reasons: [], mode: 'send', note: null, kind: 'jira', reach: null };
  return { view, button };
}

const required = [
  { value: 'summary', label: 'Summary', group: null },
  { value: 'components', label: 'Component/s', group: null },
  { value: 'environment', label: 'Environment', group: null },
];

describe('what Jira needs', () => {
  it('is asked for the project and issue type picked, for a new ticket only', async () => {
    const { view, button } = await jira();
    const values = initialValues(view);
    expect(needsAsk(view, button, values)).toBe('SP/10001');
    // an issue picked to update: Jira's create screen does not apply
    button.mappings.push({ field: 'issue', source: { kind: 'fixed', value: 'SP-12' } });
    expect(needsAsk(view, button, values)).toBeNull();
    button.mappings.pop();
    // nothing picked yet, or not Jira
    values.dropdowns.type = null;
    expect(needsAsk(view, button, values)).toBeNull();
    values.dropdowns.type = '10001';
    view.sharing[button.id]!.kind = 'github';
    expect(needsAsk(view, button, values)).toBeNull();
  });

  it('names a required field no block sends, and one whose block is empty', async () => {
    const { view, button } = await jira();
    const values = initialValues(view);
    values.texts.note = '';
    expect(unsent(view, button, values, required)).toEqual([
      'Jira needs “Component/s” for this issue type, and no block sends it',
      'Jira needs “Environment”: fill “Note”',
    ]);
    // the summary is always sent (the window's title when nothing else), a filled block is fine
    values.texts.note = 'Windows 11, RTX 4070';
    button.mappings.push({ field: 'components', source: { kind: 'fixed', value: 'Editor' } });
    expect(unsent(view, button, values, required)).toEqual([]);
  });
});
