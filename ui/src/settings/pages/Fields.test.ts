import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import Fields from './Fields.svelte';
import DataFields from './DataFields.svelte';
import { memoryApi, setApiForTests, type Api } from '../../lib/ipc';
import type { InfoType } from '../../lib/types';
import { stepLines } from '../fills';
import { go } from '../route.svelte';

let api: Api;
beforeEach(async () => {
  api = memoryApi();
  await api.saveItem('connection', 'work', { kind: 'jira', name: 'Work Jira', enabled: true, base_url: 'https://x.atlassian.net', email: 'a@b.c' } as never);
  setApiForTests(api);
});
afterEach(() => {
  cleanup();
  setApiForTests(undefined);
});

const saved = async () => (await api.listItems<InfoType>('infotype')).items;

describe('Fill fields: if a value is found, add it to a field of the service', () => {
  it('saves a step that adds two labels when a value from a file is found', async () => {
    const conn = (await api.listConnections()).items[0]!;
    render(Fields);
    await fireEvent.click(await screen.findByRole('button', { name: 'Fill a field' }));
    await fireEvent.change(screen.getByRole('combobox', { name: 'Service' }), { target: { value: conn.id } });
    await fireEvent.input(screen.getByRole('combobox', { name: 'Field' }), { target: { value: 'labels' } });
    await fireEvent.change(screen.getByRole('combobox', { name: 'When' }), { target: { value: 'if' } });
    await fireEvent.input(screen.getByRole('combobox', { name: 'What to look at' }), { target: { value: '{{rule.gpu}}' } });
    await fireEvent.input(screen.getByRole('textbox', { name: 'Text to add' }), { target: { value: 'gpu-{{rule.gpu}}' } });
    await fireEvent.click(screen.getByRole('button', { name: '+ and' }));
    await fireEvent.input(screen.getAllByRole('textbox', { name: 'Text to add' })[1]!, { target: { value: 'needs-triage' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(async () => expect((await saved()).length).toBe(1));
    const t = (await saved())[0]!.item;
    expect(t.field).toBe('labels');
    expect(t.layouts).toEqual([]);
    expect(t.steps).toEqual([
      {
        when: { subject: 'rule.gpu', test: 'found', text: '' },
        then: [{ do: 'add', text: 'gpu-{{rule.gpu}}' }, { do: 'add', text: 'needs-triage' }],
      },
    ]);
    // the card says it in a sentence
    expect(await screen.findByText('If {{rule.gpu}} is found: add “gpu-{{rule.gpu}}” and add “needs-triage”')).toBeTruthy();
  });

  it('shows a starting value from before 0.3.0 as its first step, and saves it as one', async () => {
    const conn = (await api.listConnections()).items[0]!;
    await api.saveItem('infotype', 'prio', {
      name: 'Priority', connection_id: conn.id, field: 'priority', kind: 'text', options: { type: 'none' }, required: false, default_template: 'High',
    } satisfies InfoType);
    render(Fields);
    expect(await screen.findByText('If this field is empty: set it to “High”')).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: 'Edit' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(async () => expect((await saved())[0]!.item.steps?.length).toBe(1));
    const t = (await saved())[0]!.item;
    expect(t.default_template).toBeUndefined();
    expect(t.steps![0]!.when).toEqual({ subject: '', test: 'missing', text: '' });
  });

  it('is the second tab of Data & fields, and the old address opens it', async () => {
    go('rules', 'fields');
    render(DataFields);
    expect(screen.getByRole('tab', { name: 'Fill fields' }).getAttribute('aria-selected')).toBe('true');
    await fireEvent.click(screen.getByRole('tab', { name: 'Read from files' }));
    expect(await screen.findByRole('button', { name: 'Add data from a file' })).toBeTruthy();
  });

  it('describes each kind of step in words', () => {
    const t: InfoType = {
      name: 'L', connection_id: 'c', field: 'labels', kind: 'text', options: { type: 'none' }, required: false,
      steps: [
        { when: null, then: [{ do: 'add', text: 'crash' }] },
        { when: { subject: 'app.exe', test: 'contains', text: 'maya' }, then: [{ do: 'set', text: 'dcc' }] },
      ],
    };
    expect(stepLines(t)).toEqual(['Always: add “crash”', 'If {{app.exe}} contains “maya”: set it to “dcc”']);
  });
});
