import { describe, expect, it } from 'vitest';
import { blocksOf, classify, isRule, newLine, offsetOf, parse, pointAt, readText, tableOf, tableText, toHtml } from './richtext';

const kinds = (s: string) => parse(s).map((p) => `${p.kind}:${s.slice(p.s, p.e)}`);

describe('the formatted box reads marks as the code that sends them does', () => {
  it('finds bold, italic, underline and code', () => {
    expect(kinds('a **b** *c* __d__ `e`')).toEqual(['text:a ', 'bold:**b**', 'text: ', 'italic:*c*', 'text: ', 'underline:__d__', 'text: ', 'code:`e`']);
  });

  it('counts a mark only at the edge of a word, and leaves one never closed as typed', () => {
    expect(kinds('2*3*4')).toEqual(['text:2*3*4']);
    expect(kinds('snake__case__name')).toEqual(['text:snake__case__name']);
    expect(kinds('**open')).toEqual(['text:**open']);
    expect(kinds('`a **b** c`')).toEqual(['code:`a **b** c`']);
  });

  it('knows a quote, a bullet and a numbered line, like richtext.rs', () => {
    expect(classify('> said')).toEqual({ kind: 'quote', mark: 2 });
    expect(classify('- item')).toEqual({ kind: 'bullet', mark: 2 });
    expect(classify('- ')).toEqual({ kind: 'plain', mark: 0 });
    expect(classify('12) item')).toEqual({ kind: 'number', mark: 4 });
    expect(classify('2024. was')).toEqual({ kind: 'number', mark: 6 });
  });

  it('keeps every character, so nothing is lost when it is drawn', () => {
    const html = toHtml('x <b> & **y**\n\n> q');
    const div = document.createElement('div');
    div.innerHTML = html;
    expect(Array.from(div.children).map((c) => c.textContent).join('\n')).toBe('x <b> & **y**\n\n> q');
  });

  it('Enter goes on with the list it is in', () => {
    expect(newLine('- a', 3, 3)).toEqual({ text: '- a\n- ', at: 6 });
    expect(newLine('9. a', 4, 4)).toEqual({ text: '9. a\n10. ', at: 9 });
    expect(newLine('> a', 3, 3)).toEqual({ text: '> a\n> ', at: 6 });
    expect(newLine('a\n- ', 4, 4)).toEqual({ text: 'a\n', at: 2 });
    expect(newLine('plain', 2, 2)).toEqual({ text: 'pl\nain', at: 3 });
  });
});

describe('tables and code blocks in the formatted box', () => {
  const T = '| Steps | Seen |\n| --- | --- |\n| open a \\| b | crash |';

  it('reads a table as richtext.rs does: its head, its rows, a bar kept with a backslash', () => {
    expect(tableOf(T.split('\n'))).toEqual({ head: ['Steps', 'Seen'], rows: [['open a | b', 'crash']] });
    expect(isRule('| :--- | ---: |')).toBe(true);
    expect(isRule('| a | b |')).toBe(false);
    expect(tableText({ head: ['A', 'B'], rows: [['x|y', 'z\nw']] })).toBe('| A | B |\n| --- | --- |\n| x\\|y | z w |');
  });

  it('cuts the text into lines, code blocks and tables', () => {
    const lines = ['a', '```', '**not bold**', '```', T.split('\n'), 'b', '| lonely |'].flat();
    expect(blocksOf(lines).map((b) => b.kind)).toEqual(['line', 'code', 'table', 'line', 'line']);
  });

  it('draws a table as a grid that stands for its text, and code lines as they are', () => {
    const text = `before\n${T}\n\`\`\`\nx **y**\n\`\`\`\nafter`;
    const div = document.createElement('div');
    div.innerHTML = toHtml(text);
    const grid = div.querySelector<HTMLElement>('.tw')!;
    expect(grid.getAttribute('contenteditable')).toBe('false');
    expect(Array.from(grid.querySelectorAll('.cell')).map((c) => c.textContent)).toEqual(['Steps', 'Seen', 'open a | b', 'crash']);
    expect(grid.querySelector('[data-act="addrow"]')).toBeTruthy();
    expect(grid.querySelectorAll('[data-act="col"]').length).toBe(2);
    // inside a code block nothing is a mark
    expect(div.querySelector('.ln.codeln')?.textContent).toBe('x **y**');
    expect(div.querySelector('.m')).toBeNull();
    expect(div.querySelectorAll('.ln.fence').length).toBe(2);
    // the text read back is the text, the table's included
    expect(readText(div)).toBe(text);
  });

  it('counts a table as its text when finding places, and puts a place in it at its start', () => {
    const text = `ab\n${T}\ncd`;
    const div = document.createElement('div');
    div.innerHTML = toHtml(text);
    document.body.append(div);
    const cell = div.querySelectorAll('.cell')[2]!.firstChild!;
    expect(offsetOf(div, cell, 3)).toBe(3);
    const end = text.indexOf('cd');
    const [n, o] = pointAt(div, end + 1);
    expect(offsetOf(div, n, o)).toBe(end + 1);
    expect(pointAt(div, 3)).toEqual([div, 1]);
    div.remove();
  });
});
