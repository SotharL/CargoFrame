import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import Rules from './Rules.svelte';
import { memoryApi, setApiForTests, type Api } from '../../lib/ipc';
import type { Rule, RuleTest } from '../../lib/types';

let api: Api;
let calls: Rule[];

beforeEach(() => {
  api = memoryApi();
  calls = [];
  const original = api.testRule;
  api.testRule = (rule, folder) => {
    calls.push(rule);
    return original(rule, folder);
  };
  setApiForTests(api);
});
afterEach(() => {
  cleanup();
  setApiForTests(undefined);
});

const WAIT = { timeout: 3000 };

async function openForm() {
  render(Rules);
  await fireEvent.click(await screen.findByRole('button', { name: 'Add a value from a file' }));
}
const fileInput = () => screen.getByPlaceholderText('package.json') as HTMLInputElement;
const type = (el: HTMLElement, value: string) => fireEvent.input(el, { target: { value } });

describe('Rules: live preview', () => {
  it('asks for a file first and does not try anything without one', async () => {
    await openForm();
    expect(screen.getByText(/Choose a file above/)).toBeTruthy();
    await new Promise((r) => setTimeout(r, 600));
    expect(calls).toEqual([]);
  });

  it('tries the rule by itself once a file is named, and shows the value, the file and its start', async () => {
    await openForm();
    await type(fileInput(), 'package.json');
    expect(await screen.findByText('Found', {}, WAIT)).toBeTruthy();
    expect(screen.getByText('1.2.3')).toBeTruthy();
    expect(screen.getByText('package.json', { selector: '.mono' })).toBeTruthy();
    expect(screen.getByText('Start of the file')).toBeTruthy();
    expect(screen.getByText(/"version": "1.2.3"/)).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Test' })).toBeNull();
  });

  it('waits for a pause in typing: one try for a burst of changes', async () => {
    await openForm();
    for (const text of ['p', 'pa', 'pac', 'package.json']) await type(fileInput(), text);
    await screen.findByText('Found', {}, WAIT);
    expect(calls).toHaveLength(1);
    expect(calls[0]!.file).toBe('package.json');
  });

  it('tries again when the format or the query changes', async () => {
    await openForm();
    await type(fileInput(), 'package.json');
    await screen.findByText('Found', {}, WAIT);
    await type(screen.getByLabelText('Look for'), 'version');
    await waitFor(() => expect(calls.length).toBe(2), WAIT);
    await fireEvent.change(screen.getByLabelText('Format'), { target: { value: 'file' } });
    await waitFor(() => expect(calls.length).toBe(3), WAIT);
    expect(calls[2]!.format).toBe('file');
  });

  it('shows a late answer for an older version of the rule nowhere', async () => {
    const pending: Array<(r: RuleTest) => void> = [];
    api.testRule = (rule) => {
      calls.push(rule);
      return new Promise<RuleTest>((resolve) => pending.push(resolve));
    };
    await openForm();
    await type(fileInput(), 'a.json');
    await waitFor(() => expect(pending.length).toBe(1), WAIT);
    await type(fileInput(), 'b.json');
    await waitFor(() => expect(pending.length).toBe(2), WAIT);
    pending[1]!({ folder: null, value: 'new', problem: null, file: 'b.json', excerpt: null });
    expect(await screen.findByText('new', {}, WAIT)).toBeTruthy();
    pending[0]!({ folder: null, value: 'old', problem: null, file: 'a.json', excerpt: null });
    await new Promise((r) => setTimeout(r, 50));
    expect(screen.queryByText('old')).toBeNull();
    expect(screen.getByText('new')).toBeTruthy();
  });

  it('shows a problem as a warning and keeps the file it looked at', async () => {
    api.testRule = async () => ({ folder: '/p', value: null, problem: 'Nothing found for “version”', file: 'package.json', excerpt: '{}' });
    await openForm();
    await type(fileInput(), 'package.json');
    expect(await screen.findByText('Nothing found for “version”', {}, WAIT)).toBeTruthy();
    expect(screen.queryByText('Found')).toBeNull();
    expect(screen.getByText('package.json', { selector: '.mono' })).toBeTruthy();
  });

  it('reports a failure of the preview itself as a problem', async () => {
    api.testRule = async () => {
      throw new Error('the app did not answer');
    };
    await openForm();
    await type(fileInput(), 'package.json');
    expect(await screen.findByText(/the app did not answer/, {}, WAIT)).toBeTruthy();
  });

  it('clears the preview when the file is emptied again', async () => {
    await openForm();
    await type(fileInput(), 'package.json');
    await screen.findByText('Found', {}, WAIT);
    await type(fileInput(), '');
    expect(await screen.findByText(/Choose a file above/, {}, WAIT)).toBeTruthy();
    expect(screen.queryByText('Found')).toBeNull();
  });
});

describe('Rules: the new choices', () => {
  it('offers the "any file" format without the several-values choices', async () => {
    await openForm();
    expect(screen.getByLabelText('If there are several values')).toBeTruthy();
    await fireEvent.change(screen.getByLabelText('Format'), { target: { value: 'file' } });
    expect(screen.queryByLabelText('If there are several values')).toBeNull();
    expect(screen.getByText(/sha256/)).toBeTruthy();
  });

  it('asks which one only for "the one at a number"', async () => {
    await openForm();
    expect(screen.queryByLabelText('Which one')).toBeNull();
    await fireEvent.change(screen.getByLabelText('If there are several values'), { target: { value: 'nth' } });
    expect(screen.getByLabelText('Which one')).toBeTruthy();
  });

  it('asks for the value to build only for a regular expression', async () => {
    await openForm();
    expect(screen.queryByLabelText('Build the value from groups')).toBeNull();
    await fireEvent.change(screen.getByLabelText('Format'), { target: { value: 'regex' } });
    expect(screen.getByLabelText('Build the value from groups')).toBeTruthy();
  });

  it('saves the new settings with the rule', async () => {
    await openForm();
    await type(screen.getByLabelText('Name'), 'Build');
    await type(fileInput(), 'build.log');
    await fireEvent.change(screen.getByLabelText('Format'), { target: { value: 'regex' } });
    await type(screen.getByLabelText('Look for'), 'v(\\d+)\\.(\\d+)');
    await type(screen.getByLabelText('Build the value from groups'), '$1.$2');
    await fireEvent.change(screen.getByLabelText('If there are several values'), { target: { value: 'nth' } });
    await type(screen.getByLabelText('Which one'), '-1');
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(async () => expect((await api.listItems<Rule>('rule')).items.length).toBe(1), WAIT);
    const saved = (await api.listItems<Rule>('rule')).items[0]!.item;
    expect(saved).toMatchObject({ format: 'regex', output: '$1.$2', multiple: 'nth' });
    expect(Number(saved.nth)).toBe(-1);
  });
});
