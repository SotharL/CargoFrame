import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/svelte';
import TextAutomation from './TextAutomation.svelte';
import type { ItemEntry, Rule, VariableInfo } from '../../lib/types';
import { readText, select } from '../../capture/blocks/richtext';

const variables: VariableInfo[] = [
  { name: 'app.exe', description: 'File name of the program in focus' },
  { name: 'app.path', description: 'Full path of that program' },
  { name: 'capture.screenshot', description: 'The screenshot, shown here in the text' },
  { name: 'capture.recording', description: 'The recording, shown here in the text' },
  { name: 'capture.date', description: 'Date of the capture' },
  { name: 'project.name', description: "That folder's name" },
  { name: 'app.title', description: 'The title of its window' },
];
const rules: ItemEntry<Rule>[] = [
  { id: 'build', item: { name: 'Build number', file: 'package.json', format: 'json', query: 'version', multiple: 'first', separator: ', ', nth: 1, output: '', fallback: null, max_bytes: 1048576 } },
];

const area = () => screen.getByRole('textbox', { name: 'Text automation' }) as HTMLTextAreaElement;

describe('TextAutomation', () => {
  afterEach(cleanup);

  const inGroup = (group: string, name: string) => within(screen.getByRole('group', { name: group })).getByRole('button', { name });

  it('inserts a value where the cursor is, with one click', async () => {
    render(TextAutomation, { value: 'Seen on ', variables, rules });
    area().setSelectionRange(8, 8);
    await fireEvent.click(inGroup('Most used', 'Date'));
    expect(area().value).toBe('Seen on {{capture.date}}');
  });

  it('lists every value on the right, the most used first, then by group', () => {
    render(TextAutomation, { value: '', variables, rules });
    const list = screen.getByRole('region', { name: 'Values to put in the text' });
    const groups = within(list).getAllByRole('group').map((g) => g.getAttribute('aria-label'));
    // no picture in the text for a plain box: no service shows one there
    expect(groups).toEqual(['Most used', 'Program', 'Project', 'Capture', 'From files', 'Links']);
    expect(within(screen.getByRole('group', { name: 'Program' })).getAllByRole('button').map((b) => b.textContent?.trim())).toEqual([
      'Program',
      'Program path',
      'Window title',
    ]);
  });

  it('finds a value by what it is, and inserts one read from a file', async () => {
    render(TextAutomation, { value: '', variables, rules });
    await fireEvent.input(screen.getByRole('searchbox', { name: 'Search values' }), { target: { value: 'full path' } });
    const list = screen.getByRole('region', { name: 'Values to put in the text' });
    expect(within(list).getAllByRole('button').map((b) => b.textContent?.trim())).toEqual(['Program path']);
    await fireEvent.click(inGroup('Program', 'Program path'));
    expect(area().value).toBe('{{app.path}}');
    await fireEvent.input(screen.getByRole('searchbox', { name: 'Search values' }), { target: { value: 'package.json' } });
    await fireEvent.click(within(screen.getByRole('group', { name: 'From files' })).getAllByRole('button')[0]!);
    expect(area().value).toBe('{{app.path}}{{rule.build}}');
    await fireEvent.input(screen.getByRole('searchbox', { name: 'Search values' }), { target: { value: 'nothing like it' } });
    expect(screen.getByText('No value matches “nothing like it”.')).toBeTruthy();
  });

  it('has no file group when no value is read from files', () => {
    render(TextAutomation, { value: '', variables, rules: [] });
    expect(screen.queryByRole('group', { name: 'From files' })).toBeNull();
  });

  it('for Jira, puts the screenshot and the recording in the text, or a table with both', async () => {
    render(TextAutomation, { value: 'Steps', variables, rules, marks: ['bold', 'table'], services: ['jira'] });
    const box = screen.getByRole('textbox', { name: 'Text automation' }) as HTMLElement;
    select(box, 5, 5);
    await fireEvent.click(inGroup('In the text', 'Screenshot'));
    expect(readText(box)).toBe('Steps{{capture.screenshot}}');
    await fireEvent.click(inGroup('In the text', 'Repro | Highlight table'));
    expect(readText(box)).toBe('Steps{{capture.screenshot}}\n| Repro | Highlight |\n| --- | --- |\n| {{capture.recording}} | {{capture.screenshot}} |');
    expect(box.querySelector('.tw')).toBeTruthy();
  });

  it('offers the screenshot but not the recording in a Teams message', () => {
    render(TextAutomation, { value: '', variables, rules, marks: ['bold'], services: ['teams'] });
    const g = within(screen.getByRole('group', { name: 'In the text' }));
    expect(g.getByRole('button', { name: 'Screenshot' })).toBeTruthy();
    expect(g.queryByRole('button', { name: 'Recording' })).toBeNull();
    expect(g.queryByRole('button', { name: 'Repro | Highlight table' })).toBeNull();
  });

  it('inserts a plain link', async () => {
    render(TextAutomation, { value: 'See ', variables, rules });
    area().setSelectionRange(4, 4);
    await fireEvent.click(screen.getByRole('button', { name: '🔗 Link' }));
    expect(area().value).toBe('See https://');
  });

  it('builds a custom link whose address carries live data', async () => {
    render(TextAutomation, { value: 'Fixed in ', variables, rules });
    await fireEvent.click(screen.getByRole('button', { name: '🔗 Custom link…' }));
    await fireEvent.input(screen.getByLabelText('Word that becomes the link'), { target: { value: 'build' } });
    await fireEvent.input(screen.getByLabelText('Web address'), { target: { value: 'https://ci.example.com/builds/' } });
    await fireEvent.change(screen.getByRole('combobox', { name: 'A value from a file, in the address' }), { target: { value: 'rule.build' } });
    expect(screen.getByText('[build](https://ci.example.com/builds/{{rule.build}})')).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: 'Insert link' }));
    expect(area().value).toBe('Fixed in [build](https://ci.example.com/builds/{{rule.build}})');
    expect(screen.queryByRole('group', { name: 'Custom link' })).toBeNull();
  });

  it('writes the other link styles and warns about a bad address', async () => {
    render(TextAutomation, { value: '', variables, rules });
    await fireEvent.click(screen.getByRole('button', { name: '🔗 Custom link…' }));
    await fireEvent.input(screen.getByLabelText('Word that becomes the link'), { target: { value: 'ticket' } });
    await fireEvent.input(screen.getByLabelText('Web address'), { target: { value: 'example.com/t/1' } });
    expect(screen.getByRole('note').textContent).toContain('should start with https://');
    await fireEvent.change(screen.getByLabelText('Link style'), { target: { value: 'jira' } });
    expect(screen.getByText('[ticket|example.com/t/1]')).toBeTruthy();
  });

  it('cannot insert a link that has no address yet', async () => {
    render(TextAutomation, { value: '', variables, rules });
    await fireEvent.click(screen.getByRole('button', { name: '🔗 Custom link…' }));
    expect((screen.getByRole('button', { name: 'Insert link' }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('as the message of a button, formats the text, variables and all, with the same buttons', async () => {
    render(TextAutomation, { value: '- Seen in ', variables, rules, marks: ['bold', 'code', 'italic', 'bullets'], services: ['jira'] });
    const box = screen.getByRole('textbox', { name: 'Text automation' }) as HTMLElement;
    expect(box.getAttribute('contenteditable')).toBe('true');
    select(box, 10, 10);
    await fireEvent.click(inGroup('Most used', 'Program'));
    expect(readText(box)).toBe('- Seen in {{app.exe}}');
    // the variable is shown as one, the line as a bullet
    expect(box.querySelector('.var')?.textContent).toBe('{{app.exe}}');
    expect(box.querySelector('.ln.bullet')).toBeTruthy();
    select(box, 10, 21);
    await fireEvent.click(screen.getByRole('button', { name: 'Code' }));
    expect(readText(box)).toBe('- Seen in `{{app.exe}}`');
    // only what the services can show
    expect(screen.queryByRole('button', { name: 'Underline' })).toBeNull();
  });
});
