import { describe, expect, it } from 'vitest';
import { applyMark, marksFor } from './formatting';
import { demoSession } from '../demo';
import type { Block, ServiceKind, SessionView } from '../../lib/types';

type Text = Extract<Block, { type: 'text' }>;
type Button = Extract<Block, { type: 'sharing_button' }>;

/** The demo window with its note box sent as the body of a service, or of two buttons. */
async function sentTo(...kinds: [ServiceKind, string][]): Promise<{ view: SessionView; note: Text }> {
  const view = await demoSession();
  const blocks = view.layout!.blocks;
  const note = blocks.find((b) => b.id === 'note') as Text;
  const first = blocks.find((b) => b.id === 'send') as Button;
  const buttons: Button[] = kinds.map(([kind, field], i) => ({
    ...first, id: `send${i}`, mappings: [{ field, source: { kind: 'block', value: 'note' } }],
  }));
  view.layout!.blocks = [...blocks.filter((b) => b.type !== 'sharing_button'), ...buttons];
  view.sharing = Object.fromEntries(buttons.map((b, i) => [b.id, { ...view.sharing['send']!, kind: kinds[i]![0] }]));
  return { view, note };
}

describe('marksFor', () => {
  it('offers every mark for a Jira description', async () => {
    const { view, note } = await sentTo(['jira', 'description']);
    const { marks, services } = marksFor(view, note);
    expect(marks).toEqual(['bold', 'italic', 'underline', 'code', 'quote', 'bullets', 'numbers']);
    expect(services).toEqual(['jira']);
  });

  it('leaves out underline where the service has none', async () => {
    for (const [kind, field] of [['slack', 'message'], ['reddit', 'text']] as [ServiceKind, string][]) {
      const { view, note } = await sentTo([kind, field]);
      expect(marksFor(view, note).marks, kind).not.toContain('underline');
      expect(marksFor(view, note).marks, kind).toContain('bold');
    }
  });

  it('offers only what every service the box goes to can show', async () => {
    const { view, note } = await sentTo(['jira', 'description'], ['slack', 'message']);
    const { marks, services } = marksFor(view, note);
    expect(marks).not.toContain('underline');
    expect(marks).toContain('numbers');
    expect(services).toEqual(['jira', 'slack']);
  });

  it('offers nothing for a service that takes plain text, or a box sent to a title', async () => {
    for (const [kind, field] of [['claude', 'chat'], ['google', 'title'], ['custom', 'x'], ['jira', 'summary'], ['slack', 'title']] as [ServiceKind, string][]) {
      const { view, note } = await sentTo([kind, field]);
      expect(marksFor(view, note).marks, `${kind}.${field}`).toEqual([]);
    }
  });

  it('offers nothing when no button sends the box, or the box is not plain text, or it is switched off', async () => {
    const { view, note } = await sentTo(['jira', 'description']);
    expect(marksFor(view, { ...note, id: 'elsewhere' }).marks).toEqual([]);
    expect(marksFor(view, { ...note, role: 'prompt' }).marks).toEqual([]);
    expect(marksFor(view, { ...note, formatting: false }).marks).toEqual([]);
  });
});

describe('applyMark', () => {
  const at = (text: string, a: number, b: number, mark: Parameters<typeof applyMark>[3]) => applyMark(text, a, b, mark);

  it('wraps the selection and keeps it selected', () => {
    expect(at('a crash today', 2, 7, 'bold')).toEqual({ text: 'a **crash** today', start: 4, end: 9 });
    expect(at('a crash today', 2, 7, 'italic').text).toBe('a *crash* today');
    expect(at('a crash today', 2, 7, 'underline').text).toBe('a __crash__ today');
    expect(at('a crash today', 2, 7, 'code').text).toBe('a `crash` today');
  });

  it('takes the mark off again, whether it is outside the selection or inside it', () => {
    expect(at('a **crash** today', 4, 9, 'bold')).toEqual({ text: 'a crash today', start: 2, end: 7 });
    expect(at('a **crash** today', 2, 11, 'bold')).toEqual({ text: 'a crash today', start: 2, end: 7 });
  });

  it('puts a pair of marks in for an empty selection, with the cursor between them', () => {
    expect(at('ab', 1, 1, 'bold')).toEqual({ text: 'a****b', start: 3, end: 3 });
    expect(at('', 0, 0, 'code')).toEqual({ text: '``', start: 1, end: 1 });
  });

  it('makes a list of the lines the selection touches, counting from one', () => {
    const text = 'Steps:\nopen\nsave\nquit';
    expect(at(text, 8, 14, 'numbers').text).toBe('Steps:\n1. open\n2. save\nquit');
    expect(at(text, 8, 14, 'bullets').text).toBe('Steps:\n- open\n- save\nquit');
    expect(at(text, 0, 0, 'quote').text).toBe('> Steps:\nopen\nsave\nquit');
  });

  it('takes a list off when every line has it, and turns one kind of list into another', () => {
    expect(at('- a\n- b', 0, 7, 'bullets').text).toBe('a\nb');
    expect(at('- a\n- b', 0, 7, 'numbers').text).toBe('1. a\n2. b');
    expect(at('- a\nb', 0, 5, 'bullets').text).toBe('- a\n- b');
  });

  it('works on the last line and on one line of a longer text', () => {
    expect(at('one\ntwo', 5, 5, 'quote').text).toBe('one\n> two');
    expect(at('one\ntwo\nthree', 4, 4, 'bullets').text).toBe('one\n- two\nthree');
  });
});
