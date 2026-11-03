import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import TextAutomation from './TextAutomation.svelte';
import type { ItemEntry, Rule, VariableInfo } from '../../lib/types';

const variables: VariableInfo[] = [
  { name: 'app.exe', description: 'File name of the program in focus' },
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

  it('inserts a value where the cursor is, with one click', async () => {
    render(TextAutomation, { value: 'Seen on ', variables, rules });
    area().setSelectionRange(8, 8);
    await fireEvent.click(screen.getByRole('button', { name: '＋ Date' }));
    expect(area().value).toBe('Seen on {{capture.date}}');
  });

  it('offers the other values and the values from files in drop downs', async () => {
    render(TextAutomation, { value: '', variables, rules });
    await fireEvent.change(screen.getByRole('combobox', { name: 'More values' }), { target: { value: 'app.title' } });
    expect(area().value).toBe('{{app.title}}');
    await fireEvent.change(screen.getByRole('combobox', { name: 'A value from a file' }), { target: { value: 'build' } });
    expect(area().value).toBe('{{app.title}}{{rule.build}}');
  });

  it('hides the file drop down when there are no values from files', () => {
    render(TextAutomation, { value: '', variables, rules: [] });
    expect(screen.queryByRole('combobox', { name: 'A value from a file' })).toBeNull();
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
    await fireEvent.change(screen.getByRole('combobox', { name: 'A value from a file, in the address' }), { target: { value: 'build' } });
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
});
