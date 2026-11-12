import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import TextBlock from './TextBlock.svelte';
import type { Block } from '../../lib/types';
import { readText, select, selectionIn } from './richtext';

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

  it('keeps its box one line high when the layout asks, with or without formatting', () => {
    render(TextBlock, { block: { ...block('above'), one_line: true }, value: '' });
    expect(document.querySelector('.field')?.classList.contains('one')).toBe(true);
    cleanup();
    render(TextBlock, { block: { ...block('above'), one_line: true }, value: '', marks: ['bold'], services: ['jira'] });
    expect(document.querySelector('.field')?.classList.contains('one')).toBe(true);
    cleanup();
    render(TextBlock, { block: block('above'), value: '' });
    expect(document.querySelector('.field')?.classList.contains('one')).toBe(false);
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

  const rich = () => screen.getByRole('textbox') as HTMLDivElement;
  const selected = () => selectionIn(rich());
  const pick = (a: number, b: number) => select(rich(), a, b);

  it('shows the marks as they look, and hides their stars until the cursor is at them', () => {
    render(TextBlock, { block: block('above'), value: 'it **crashes** on save\n- open\n> said', marks: [...tools], services: ['jira'] });
    const box = rich();
    expect(box.getAttribute('contenteditable')).toBe('true');
    expect(screen.getByRole('textbox', { name: 'Description' })).toBe(box);
    expect(box.querySelector('.m.bold')?.textContent).toBe('**crashes**');
    expect(box.querySelector('.m.bold.on')).toBeNull();
    expect(box.querySelector('.ln.bullet')).toBeTruthy();
    expect(box.querySelector('.ln.quote')).toBeTruthy();
    // every character is in the box, so the text read back is the text
    expect(readText(box)).toBe('it **crashes** on save\n- open\n> said');
  });

  it('a button marks the selected words and keeps them selected', async () => {
    render(TextBlock, { block: block('above'), value: 'it crashes on save', marks: [...tools], services: ['jira'] });
    pick(3, 10);
    await fireEvent.click(screen.getByRole('button', { name: names.bold }));
    expect(readText(rich())).toBe('it **crashes** on save');
    expect(selected()).toEqual([5, 12]);
    // the selection is in the bold words: their stars show
    expect(rich().querySelector('.m.bold.on')).toBeTruthy();
    // pressed again, the words are plain again
    await fireEvent.click(screen.getByRole('button', { name: names.bold }));
    expect(readText(rich())).toBe('it crashes on save');
  });

  it('a list button works on the line the cursor is in', async () => {
    render(TextBlock, { block: block('above'), value: 'Steps:\nopen\nsave', marks: [...tools], services: ['jira'] });
    pick(8, 16);
    await fireEvent.click(screen.getByRole('button', { name: names.numbers }));
    expect(readText(rich())).toBe('Steps:\n1. open\n2. save');
    expect(rich().querySelectorAll('.ln.number').length).toBe(2);
  });

  it('Ctrl+B, Ctrl+E and Ctrl+Shift+8 do the same, but only for marks on offer', async () => {
    render(TextBlock, { block: block('above'), value: 'word', marks: ['bold', 'code', 'bullets'], services: ['slack'] });
    pick(0, 4);
    await fireEvent.keyDown(rich(), { key: 'b', code: 'KeyB', ctrlKey: true });
    expect(readText(rich())).toBe('**word**');
    pick(0, 8);
    await fireEvent.keyDown(rich(), { key: 'u', code: 'KeyU', ctrlKey: true });
    // no underline on offer: nothing happens
    expect(readText(rich())).toBe('**word**');
    await fireEvent.keyDown(rich(), { key: '*', code: 'Digit8', ctrlKey: true, shiftKey: true });
    expect(readText(rich())).toBe('- **word**');
  });

  it('Enter goes on with a list, and ends it on an empty item', async () => {
    render(TextBlock, { block: block('above'), value: '1. open', marks: [...tools], services: ['jira'] });
    pick(7, 7);
    await fireEvent(rich(), new InputEvent('beforeinput', { inputType: 'insertParagraph', cancelable: true, bubbles: true }));
    expect(readText(rich())).toBe('1. open\n2. ');
    expect(selected()).toEqual([11, 11]);
    await fireEvent(rich(), new InputEvent('beforeinput', { inputType: 'insertParagraph', cancelable: true, bubbles: true }));
    expect(readText(rich())).toBe('1. open\n');
  });

  it('what is typed is read back and drawn with its marks; Ctrl+Z takes it back', async () => {
    render(TextBlock, { block: block('above'), value: 'a *b', marks: [...tools], services: ['jira'] });
    const box = rich();
    // the browser types the closing star in the last text of the box
    const walk = document.createTreeWalker(box, NodeFilter.SHOW_TEXT);
    let last: CharacterData | null = null;
    for (let t = walk.nextNode(); t; t = walk.nextNode()) last = t as CharacterData;
    pick(4, 4);
    await fireEvent(box, new InputEvent('beforeinput', { inputType: 'insertText', data: '*', cancelable: true, bubbles: true }));
    last!.data += '*';
    await fireEvent.input(box);
    expect(readText(box)).toBe('a *b*');
    expect(box.querySelector('.m.italic')).toBeTruthy();
    await fireEvent.keyDown(box, { key: 'z', code: 'KeyZ', ctrlKey: true });
    expect(readText(box)).toBe('a *b');
  });

  it('a plain box stays a plain text area', () => {
    render(TextBlock, { block: block('above'), value: 'x' });
    expect((screen.getByRole('textbox') as HTMLElement).tagName).toBe('TEXTAREA');
  });
});

describe('TextBlock tables', () => {
  afterEach(cleanup);
  const T = '| A | B |\n| --- | --- |\n| 1 | 2 |';
  const rich = () => screen.getByRole('textbox') as HTMLDivElement;
  const cells = () => Array.from(rich().querySelectorAll<HTMLElement>('.cell'));

  it('the Table button puts a table to fill in after the line, the cursor in its first cell', async () => {
    render(TextBlock, { block: block('above'), value: 'Steps', marks: ['bold', 'table'], services: ['jira'] });
    select(rich(), 2, 2);
    await fireEvent.click(screen.getByRole('button', { name: 'Table' }));
    expect(readText(rich())).toBe('Steps\n| Column 1 | Column 2 |\n| --- | --- |\n|  |  |');
    expect(document.activeElement).toBe(cells()[0]);
  });

  it('typing in a cell changes that cell of the table in the text', async () => {
    render(TextBlock, { block: block('above'), value: `x\n${T}\ny`, marks: ['bold', 'table'], services: ['jira'] });
    const c = cells()[3]!;
    c.textContent = '2 then 3';
    await fireEvent.input(c);
    expect(readText(rich())).toBe('x\n| A | B |\n| --- | --- |\n| 1 | 2 then 3 |\ny');
  });

  it('adds and takes out rows and columns with its buttons', async () => {
    render(TextBlock, { block: block('above'), value: T, marks: ['table'], services: ['jira'] });
    await fireEvent.click(rich().querySelector('[data-act="addrow"]')!);
    expect(readText(rich())).toBe('| A | B |\n| --- | --- |\n| 1 | 2 |\n|  |  |');
    await fireEvent.click(rich().querySelector('[data-act="addcol"]')!);
    expect(readText(rich())).toBe('| A | B | Column 3 |\n| --- | --- | --- |\n| 1 | 2 |  |\n|  |  |  |');
    await fireEvent.click(rich().querySelector('[data-act="col"][data-n="0"]')!);
    expect(readText(rich())).toBe('| B | Column 3 |\n| --- | --- |\n| 2 |  |\n|  |  |');
    await fireEvent.click(rich().querySelector('[data-act="row"][data-n="1"]')!);
    expect(readText(rich())).toBe('| B | Column 3 |\n| --- | --- |\n| 2 |  |');
  });

  it('a button pressed with the cursor in a cell marks the cell text', async () => {
    render(TextBlock, { block: block('above'), value: T, marks: ['bold', 'table'], services: ['jira'] });
    const c = cells()[2]!;
    c.focus();
    const r = document.createRange();
    r.selectNodeContents(c);
    document.getSelection()!.removeAllRanges();
    document.getSelection()!.addRange(r);
    document.dispatchEvent(new Event('selectionchange'));
    await fireEvent.click(screen.getByRole('button', { name: 'Bold' }));
    expect(readText(rich())).toBe('| A | B |\n| --- | --- |\n| **1** | 2 |');
  });
});
