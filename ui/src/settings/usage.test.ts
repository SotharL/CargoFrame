import { describe, expect, it } from 'vitest';
import { layoutsUsingField, layoutsUsingRule, usedIn } from './usage';
import type { Block, ItemEntry, Layout } from '../lib/types';

const rect = { x: 0, y: 0, w: 4, h: 2 };
const text = (id: string, default_text: string): Block => ({
  id, rect, type: 'text', title: 'Note', placement: 'above', role: 'plain', default_text,
});
const button = (id: string, mappings: Block extends infer B ? (B extends { mappings: infer M } ? M : never) : never): Block => ({
  id, rect, type: 'sharing_button', label: 'Share', connection_id: null, prompt_block_id: null, mappings, on_success: 'close_window',
});
const layout = (id: string, name: string, blocks: Block[]): ItemEntry<Layout> => ({ id, item: { id, name, blocks } });

const layouts = [
  layout('bug', 'Bug report', [text('t', 'Build {{rule.build}} on {{capture.date}}'), button('s', [{ field: 'priority', source: { kind: 'fixed', value: 'High' } }])]),
  layout('note', 'Daily note', [button('s', [{ field: 'summary', source: { kind: 'rule', value: 'build' } }])]),
  layout('empty', '', [text('t', 'nothing here')]),
];

describe('where a rule is used', () => {
  it('finds a rule written in a text block or picked as a field source', () => {
    expect(layoutsUsingRule(layouts, 'build')).toEqual(['Bug report', 'Daily note']);
  });
  it('does not mistake a longer name for the same rule', () => {
    expect(layoutsUsingRule(layouts, 'bui')).toEqual([]);
  });
});

describe('where a destination field is used', () => {
  it('finds sharing buttons that fill it', () => {
    expect(layoutsUsingField(layouts, 'priority')).toEqual(['Bug report']);
    expect(layoutsUsingField(layouts, 'nothing')).toEqual([]);
  });
});

describe('the sentence', () => {
  it('says so when nothing uses it', () => {
    expect(usedIn([])).toBe('Not used by any layout yet');
  });
  it('lists the first three and counts the rest', () => {
    expect(usedIn(['A', 'B'])).toBe('Used in A, B');
    expect(usedIn(['A', 'B', 'C', 'D', 'E'])).toBe('Used in A, B, C and 2 more');
  });
  it('falls back to the id for a layout without a name', () => {
    expect(layoutsUsingField([layout('x', '  ', [button('s', [{ field: 'f', source: { kind: 'block', value: 'p' } }])])], 'f')).toEqual(['x']);
  });
});
