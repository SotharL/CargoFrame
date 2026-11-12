import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import Rules from './Rules.svelte';
import { memoryApi, setApiForTests, type Api } from '../../lib/ipc';
import type { Rule, RuleTest } from '../../lib/types';
import { slug, unique, valuesOf } from '../ruleValues';

let api: Api;
let calls: Rule[];

beforeEach(() => {
  api = memoryApi();
  calls = [];
  const original = api.testRule;
  api.testRule = (rule, folder) => {
    calls.push(JSON.parse(JSON.stringify(rule)) as Rule);
    return original(rule, folder);
  };
  setApiForTests(api);
});
afterEach(() => {
  cleanup();
  setApiForTests(undefined);
});

const WAIT = { timeout: 3000 };
const type = (el: HTMLElement, value: string) => fireEvent.input(el, { target: { value } });

async function openForm() {
  render(Rules);
  await fireEvent.click(await screen.findByRole('button', { name: 'Add data from a file' }));
}
const fileBox = () => screen.getByRole('textbox', { name: 'File name, or part of it' });
const nameBoxes = () => screen.getAllByRole('textbox', { name: 'Name of the value' });

describe('Data from files: if a file is found, read values', () => {
  it('asks for no format: a file name and a value with what is written before it', async () => {
    await openForm();
    expect(screen.queryByRole('combobox', { name: 'Format' })).toBeNull();
    await type(screen.getByPlaceholderText('Machine config'), 'Machine');
    await type(fileBox(), 'system_info');
    await type(nameBoxes()[0]!, 'GPU');
    // the value's variable follows its name, and it looks for that name
    expect(screen.getByRole('button', { name: '{{rule.gpu}}' })).toBeTruthy();
    expect((screen.getByRole('textbox', { name: 'The name written before the value' }) as HTMLInputElement).value).toBe('GPU');
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(async () => expect((await api.listItems<Rule>('rule')).items.length).toBe(1));
    const saved = (await api.listItems<Rule>('rule')).items[0]!.item;
    expect(saved.file).toBe('system_info');
    expect(saved.values).toEqual([
      { key: 'gpu', name: 'GPU', find: { type: 'value_of', name: 'GPU' }, which: 'first', fallback: null },
    ]);
  });

  it('reads the text between two pieces, to the end of the line unless an end is given', async () => {
    await openForm();
    await type(fileBox(), 'app.log');
    await type(nameBoxes()[0]!, 'Build');
    await fireEvent.change(screen.getByRole('combobox', { name: 'How to find it' }), { target: { value: 'between' } });
    // what was looked for stays as the text before
    expect((screen.getByRole('textbox', { name: 'The text before the value' }) as HTMLInputElement).value).toBe('Build');
    expect(screen.getByRole('button', { name: 'the end of the line' })).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: 'the end of the line' }));
    await type(screen.getByRole('textbox', { name: 'The text after the value' }), ']');
    await fireEvent.change(screen.getByRole('combobox', { name: 'Which one' }), { target: { value: 'last' } });
    await waitFor(() => {
      const v = calls.at(-1)?.values?.[0];
      expect(v?.find).toEqual({ type: 'between', start: 'Build', end: ']' });
      expect(v?.which).toBe('last');
    }, WAIT);
  });

  it('can add the file to the files only, or read several values', async () => {
    await openForm();
    await type(screen.getByPlaceholderText('Machine config'), 'Logs');
    await type(fileBox(), '.log');
    await fireEvent.change(screen.getByRole('combobox', { name: 'If several files match' }), { target: { value: 'all' } });
    await fireEvent.click(screen.getByRole('checkbox', { name: /Add it to the files/ }));
    await fireEvent.click(screen.getByRole('button', { name: 'Remove this value' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(async () => expect((await api.listItems<Rule>('rule')).items.length).toBe(1));
    const saved = (await api.listItems<Rule>('rule')).items[0]!.item;
    expect(saved.add_to_files).toBe(true);
    expect(saved.several).toBe('all');
    expect(saved.values).toEqual([]);
  });

  it('can only say how a log writes its time, for the log viewers, with what a time looks like', async () => {
    await openForm();
    await type(screen.getByPlaceholderText('Machine config'), 'Game log');
    await type(fileBox(), 'Game.log');
    await fireEvent.click(screen.getByRole('button', { name: 'Remove this value' }));
    const format = screen.getByRole('combobox', { name: "In a log viewer, its lines' time looks like" });
    await type(format, '[YYYY_MM_DD] HH:mm -');
    expect(screen.getByText(/Lines start like/).textContent).toMatch(/\[\d{4}_\d{2}_\d{2}\] \d{2}:\d{2} -/);
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(async () => expect((await api.listItems<Rule>('rule')).items.length).toBe(1));
    const saved = (await api.listItems<Rule>('rule')).items[0]!.item;
    expect(saved.log_time).toBe('[YYYY_MM_DD] HH:mm -');
    expect(saved.values).toEqual([]);
    // the list says it
    expect(await screen.findByText(/read its lines' time as/)).toBeTruthy();
  });

  it('gives two values two variables', async () => {
    await openForm();
    await type(nameBoxes()[0]!, 'GPU');
    await fireEvent.click(screen.getByRole('button', { name: 'Add a value to read' }));
    await type(nameBoxes()[1]!, 'GPU');
    expect(screen.getByRole('button', { name: '{{rule.gpu}}' })).toBeTruthy();
    expect(screen.getByRole('button', { name: '{{rule.gpu_2}}' })).toBeTruthy();
  });

  it('shows what each value gives, from the file found', async () => {
    await openForm();
    await type(fileBox(), 'system_info');
    await type(nameBoxes()[0]!, 'GPU');
    expect(await screen.findByText('GPU demo', undefined, WAIT)).toBeTruthy();
    expect(screen.getByText(/read as text/)).toBeTruthy();
  });

  it('shows a rule from before 0.3.0 with its one value, named as it always was', async () => {
    await api.saveItem('rule', 'version', {
      name: 'Version', file: 'package.json', format: 'json', query: 'version', multiple: 'first', separator: ', ', nth: 1, output: '', fallback: null, max_bytes: 1048576,
    } satisfies Rule);
    render(Rules);
    const card = (await screen.findByText('Version')).closest('article')!;
    expect(within(card as HTMLElement).getByText('{{rule.version}}')).toBeTruthy();
    await fireEvent.click(within(card as HTMLElement).getByRole('button', { name: 'Edit' }));
    expect(screen.getByText(/Made before 0.3.0/)).toBeTruthy();
    expect(screen.getByRole('button', { name: '{{rule.version}}' })).toBeTruthy();
  });

  it('a late answer for an older version of the rule is shown nowhere', async () => {
    let release: (t: RuleTest) => void = () => {};
    const original = api.testRule;
    let first = true;
    api.testRule = (rule, folder) => {
      if (first) {
        first = false;
        return new Promise((r) => (release = r));
      }
      return original(rule, folder);
    };
    await openForm();
    await type(fileBox(), 'a.txt');
    await new Promise((r) => setTimeout(r, 500));
    await type(fileBox(), 'b.txt');
    await screen.findByText('b.txt', undefined, WAIT);
    release({ folder: null, value: null, problem: null, file: 'a.txt', excerpt: null });
    await new Promise((r) => setTimeout(r, 50));
    expect(screen.queryByText('a.txt')).toBeNull();
  });
});

describe('Data from files: names', () => {
  it('makes a value from other values, with choices tried from the top and conditions', async () => {
    render(Rules);
    await fireEvent.click(await screen.findByRole('button', { name: 'Make a value from other values' }));
    // no file to name: it reads none
    expect(screen.queryByRole('textbox', { name: 'File name, or part of it' })).toBeNull();
    await type(screen.getByPlaceholderText('Machine config'), 'Build');
    await type(nameBoxes()[0]!, 'Build id');
    await type(screen.getByRole('combobox', { name: 'Choice 1' }), '{{rule.stream}}_{{rule.change}}_{{rule.shelf}}_{{rule.config}}');
    await fireEvent.click(screen.getByRole('button', { name: 'Add a choice' }));
    await type(screen.getByRole('combobox', { name: 'Choice 2' }), '{{rule.stream}}_{{rule.config}}');
    await fireEvent.click(screen.getAllByRole('button', { name: 'Add a condition' })[1]!);
    await type(screen.getByRole('combobox', { name: 'Value condition 1 of choice 2 looks at' }), 'rule.config');
    await fireEvent.change(screen.getByRole('combobox', { name: 'Test of condition 1 of choice 2' }), { target: { value: 'is' } });
    await type(screen.getByRole('textbox', { name: 'Text of condition 1 of choice 2' }), 'Debug');
    // the more precise one first; the order can be changed
    await fireEvent.click(screen.getByRole('button', { name: 'Move choice 2 up' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Move choice 1 down' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(async () => expect((await api.listItems<Rule>('rule')).items.length).toBe(1));
    const saved = (await api.listItems<Rule>('rule')).items[0]!.item;
    expect(saved.file).toBe('');
    expect(saved.values![0]).toMatchObject({
      key: 'build_id',
      find: {
        type: 'combine',
        choices: [
          { text: '{{rule.stream}}_{{rule.change}}_{{rule.shelf}}_{{rule.config}}' },
          { text: '{{rule.stream}}_{{rule.config}}', when: [{ value: 'rule.config', test: 'is', text: 'Debug' }] },
        ],
      },
    });
    expect(await screen.findByText('Made from other values, no file read')).toBeTruthy();
  });

  it('makes variable names from what people write, and keeps them apart', () => {
    expect(slug('OS Name')).toBe('os_name');
    expect(slug('Carte graphique (GPU)')).toBe('carte_graphique_gpu');
    expect(slug('')).toBe('value');
    expect(unique('gpu', ['gpu', 'gpu_2'])).toBe('gpu_3');
    expect(valuesOf('v', { name: 'V', file: 'a', format: 'json', query: '', multiple: 'first', separator: '', nth: 1, max_bytes: 1 })).toEqual([]);
  });
});
