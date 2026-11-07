import { beforeEach, describe, expect, it } from 'vitest';
import { rememberChoices } from './remembering';
import { demoSession } from './demo';
import { initialValues } from './values';
import { deeperKey, recentChoices } from '../lib/recent';

async function pathView() {
  const view = await demoSession();
  const where = view.layout!.blocks.find((b) => b.id === 'proj')!;
  Object.assign(where, { deeper: { list: 'threads', field: 'thread' } });
  view.dropdowns['proj'] = {
    status: 'ready', selected: 'C1', message: null,
    options: [{ value: 'C1', label: '#bugs', group: null }, { value: 'C2', label: '#help', group: null }],
  };
  return view;
}

describe('rememberChoices', () => {
  beforeEach(() => localStorage.clear());

  it('keeps the channel chosen, and nothing for a dropdown with nothing chosen', async () => {
    const view = await pathView();
    const values = initialValues(view);
    values.dropdowns['proj'] = 'C1';
    rememberChoices(view, values);
    expect(recentChoices('bug', 'proj')).toEqual([{ value: 'C1', label: '#bugs' }]);
    // the thread step was never filled: nothing is kept inside the channel
    expect(recentChoices('bug', deeperKey('proj', 'C1'))).toEqual([]);
  });

  it('keeps a post picked inside the channel, under that channel only', async () => {
    const view = await pathView();
    const values = initialValues(view);
    values.dropdowns['proj'] = 'C1';
    values.deeper['proj'] = { value: 'p1', label: 'Slow start' };
    rememberChoices(view, values);
    expect(recentChoices('bug', deeperKey('proj', 'C1'))).toEqual([{ value: 'p1', label: 'Slow start' }]);
    expect(recentChoices('bug', deeperKey('proj', 'C2'))).toEqual([]);
  });

  it('keeps the post a share made, in place of "new", so it can be posted in next time', async () => {
    const view = await pathView();
    const values = initialValues(view);
    values.dropdowns['proj'] = 'C2';
    values.deeper['proj'] = { value: '', label: 'New message or post' };
    rememberChoices(view, values, { value: '555', label: 'Crash on save', group: null });
    expect(recentChoices('bug', deeperKey('proj', 'C2'))).toEqual([{ value: '555', label: 'Crash on save' }]);
  });

  it('keeps nothing for a block that is told not to remember', async () => {
    const view = await pathView();
    Object.assign(view.layout!.blocks.find((b) => b.id === 'proj')!, { remember: false });
    const values = initialValues(view);
    values.dropdowns['proj'] = 'C1';
    rememberChoices(view, values, { value: '555', label: 'x', group: null });
    expect(recentChoices('bug', 'proj')).toEqual([]);
    expect(recentChoices('bug', deeperKey('proj', 'C1'))).toEqual([]);
  });
});
