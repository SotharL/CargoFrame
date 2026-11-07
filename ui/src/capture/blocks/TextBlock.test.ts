import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import TextBlock from './TextBlock.svelte';
import type { Block } from '../../lib/types';

type Text = Extract<Block, { type: 'text' }>;
const block = (placement: Text['placement'], role: Text['role'] = 'plain'): Text => ({
  id: 'note', type: 'text', rect: { x: 0, y: 0, w: 4, h: 2 }, title: 'Description', placement, role, default_text: '',
});

describe('TextBlock title', () => {
  afterEach(cleanup);

  it('sits above the box when asked to', () => {
    render(TextBlock, { block: block('above'), value: '' });
    expect(screen.getByLabelText('Description')).toBeTruthy();
    expect(document.querySelector('.inner')).toBeNull();
  });

  it('inside the box: shown while empty, gone as soon as there is text, still the tooltip', async () => {
    render(TextBlock, { block: block('inside', 'prompt'), value: '' });
    const box = screen.getByRole('textbox', { name: 'Description' }) as HTMLTextAreaElement;
    expect(document.querySelector('.inner')?.textContent).toContain('Description');
    expect(document.querySelector('.inner')?.textContent).toContain('Prompt');

    await fireEvent.input(box, { target: { value: 'hello' } });
    expect(document.querySelector('.inner')).toBeNull();
    // No bubble on the box's edge (it was cut off at the top); the tooltip is the native one.
    expect(document.querySelector('.tab')).toBeNull();
    expect(document.querySelector('.field')?.getAttribute('title')).toBe('Description');
  });

  it('wears a small tag for the roles that mean something', () => {
    render(TextBlock, { block: block('above', 'zip_name'), value: '' });
    expect(document.querySelector('.role')?.textContent).toBe('Zip name');
    cleanup();
    render(TextBlock, { block: block('above', 'plain'), value: '' });
    expect(document.querySelector('.role')).toBeNull();
  });

  it('keeps the box named for screen readers either way', async () => {
    render(TextBlock, { block: block('inside'), value: 'already filled in' });
    expect(screen.getByRole('textbox', { name: 'Description' })).toBeTruthy();
  });
});

describe('TextBlock formatting buttons', () => {
  afterEach(cleanup);
  const tools = ['bold', 'italic', 'underline', 'code', 'quote', 'bullets', 'numbers'] as const;
  const names = { bold: 'Bold', italic: 'Italic', underline: 'Underline', code: 'Code', quote: 'Quote', bullets: 'Bulleted list', numbers: 'Numbered list' };

  it('has none unless there is something to format for', () => {
    render(TextBlock, { block: block('above'), value: '' });
    expect(screen.queryByRole('toolbar')).toBeNull();
  });

  it('shows the buttons it is given, and says which services they are for', () => {
    render(TextBlock, { block: block('above'), value: '', marks: ['bold', 'code'], services: ['jira', 'slack'] });
    expect(screen.getByRole('toolbar', { name: 'Formatting for Jira, Slack' })).toBeTruthy();
    expect(screen.getAllByRole('button').map((b) => b.getAttribute('aria-label'))).toEqual(['Bold', 'Code']);
  });

  it('a button marks the selected words and keeps them selected', async () => {
    render(TextBlock, { block: block('above'), value: 'it crashes on save', marks: [...tools], services: ['jira'] });
    const box = screen.getByRole('textbox') as HTMLTextAreaElement;
    box.setSelectionRange(3, 10);
    await fireEvent.click(screen.getByRole('button', { name: names.bold }));
    expect(box.value).toBe('it **crashes** on save');
    await new Promise((r) => setTimeout(r, 0));
    expect([box.selectionStart, box.selectionEnd]).toEqual([5, 12]);
  });

  it('a list button works on the line the cursor is in', async () => {
    render(TextBlock, { block: block('above'), value: 'Steps:\nopen\nsave', marks: [...tools], services: ['jira'] });
    const box = screen.getByRole('textbox') as HTMLTextAreaElement;
    box.setSelectionRange(8, 16);
    await fireEvent.click(screen.getByRole('button', { name: names.numbers }));
    expect(box.value).toBe('Steps:\n1. open\n2. save');
  });

  it('Ctrl+B, Ctrl+I and Ctrl+U do the same, but only for marks on offer', async () => {
    render(TextBlock, { block: block('above'), value: 'word', marks: ['bold', 'italic'], services: ['slack'] });
    const box = screen.getByRole('textbox') as HTMLTextAreaElement;
    box.setSelectionRange(0, 4);
    await fireEvent.keyDown(box, { key: 'b', ctrlKey: true });
    expect(box.value).toBe('**word**');
    box.setSelectionRange(0, 8);
    await fireEvent.keyDown(box, { key: 'u', ctrlKey: true });
    // no underline on offer: the keys are left to the box
    expect(box.value).toBe('**word**');
  });
});

