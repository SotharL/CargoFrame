import { describe, expect, it } from 'vitest';
import type { Block } from '../../lib/types';
import { canUpdate, fillable, setRequired, setUpdate, updateOf, updateOptions } from './updates';

type SharingBlock = Extract<Block, { type: 'sharing_button' }>;
const text = (id: string): Block => ({ id, rect: { x: 0, y: 0, w: 1, h: 1 }, type: 'text', title: id, placement: 'above', role: 'plain', default_text: '' } as Block);
const tags = (id: string): Block => ({ id, rect: { x: 0, y: 0, w: 1, h: 1 }, type: 'tags', title: id, tags: [] } as Block);
const button = (): SharingBlock => ({
  id: 'send', rect: { x: 0, y: 0, w: 1, h: 1 }, type: 'sharing_button', label: 'Share', connection_id: 'c', prompt_block_id: null,
  on_success: 'close_window',
  mappings: [
    { field: 'summary', source: { kind: 'block', value: 'summary' } },
    { field: 'labels', source: { kind: 'block', value: 'labels' } },
  ],
} as SharingBlock);

describe('update modes', () => {
  it('are offered for Jira and GitHub only', () => {
    expect(canUpdate('jira')).toBe(true);
    expect(canUpdate('github')).toBe(true);
    expect(canUpdate('slack')).toBe(false);
    expect(canUpdate(null)).toBe(false);
  });

  it('offer what makes sense for the block and the field', () => {
    const modes = (b: Block, f: string) => updateOptions(b, f).map((o) => o.value);
    expect(modes(text('d'), 'description')).toEqual(['replace', 'append', 'append_line', 'skip']);
    expect(modes(tags('l'), 'labels')).toEqual(['replace', 'add', 'skip']);
    expect(modes(text('n'), 'customfield_10042')).toEqual(['replace', 'append', 'append_line', 'increment', 'skip']);
    // the fields that pick the issue, or the comment, are not updated
    expect(modes(text('c'), 'comment')).toEqual([]);
    expect(modes(text('i'), 'issue')).toEqual([]);
  });

  it('are saved on the mapping, replace being left unsaid', () => {
    const b = button();
    b.mappings = setUpdate(b, 'labels', 'add');
    expect(updateOf(b, 'labels')).toBe('add');
    expect(updateOf(b, 'summary')).toBe('replace');
    b.mappings = setUpdate(b, 'labels', 'replace');
    expect(b.mappings[1]).toEqual({ field: 'labels', source: { kind: 'block', value: 'labels' } });
  });
});

describe('blocks that must be filled', () => {
  it('are the ones a person fills, in the order of the window', () => {
    const blocks = [text('a'), tags('b'), button() as Block, text('c')];
    expect(fillable(blocks).map((b) => b.id)).toEqual(['a', 'b', 'c']);
    const b = button();
    b.required = setRequired(b, blocks, 'c', true);
    b.required = setRequired(b, blocks, 'a', true);
    expect(b.required).toEqual(['a', 'c']);
    expect(setRequired(b, blocks, 'a', false)).toEqual(['c']);
    // a block taken away is dropped
    expect(setRequired({ ...b, required: ['gone', 'a'] }, blocks, 'b', true)).toEqual(['a', 'b']);
  });
});
