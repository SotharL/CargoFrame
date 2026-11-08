import { describe, expect, it } from 'vitest';
import { classify, newLine, parse, toHtml } from './richtext';

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
