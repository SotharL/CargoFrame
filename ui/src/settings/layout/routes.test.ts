import { describe, expect, it } from 'vitest';
import { applyRoute, currentRoute, isAutomatic, NOT_SENT, PROMPT, routeOptions } from './routes';
import type { Block } from '../../lib/types';

type Button = Extract<Block, { type: 'sharing_button' }>;

const blocks: Block[] = [
  { id: 'prompt', type: 'text', rect: { x: 0, y: 0, w: 4, h: 2 }, title: 'Prompt', placement: 'above', role: 'prompt', default_text: '' },
  { id: 'note', type: 'text', rect: { x: 0, y: 2, w: 4, h: 2 }, title: 'Note', placement: 'above', role: 'plain', default_text: '' },
  { id: 'proj', type: 'dropdown', rect: { x: 4, y: 0, w: 4, h: 2 }, title: 'Project', source: { type: 'claude_chats' } },
];
const button = (patch: Partial<Button> = {}): Button => ({
  id: 'send', type: 'sharing_button', rect: { x: 4, y: 2, w: 4, h: 1 }, label: 'Go', connection_id: 'jira',
  prompt_block_id: null, mappings: [], on_success: 'keep_open', ...patch,
});
const fields = [{ key: 'summary', label: 'Summary' }, { key: 'description', label: 'Description' }, { key: 'project', label: 'Project' }];
const field = (f: string, block: string) => ({ field: f, source: { kind: 'block' as const, value: block } });

describe('what a button does with a block, as a choice', () => {
  it('reads it from what the button really does', () => {
    expect(currentRoute({ block_id: 'n', state: 'sent', role: 'prompt' })).toBe(PROMPT);
    expect(currentRoute({ block_id: 'n', state: 'sent', role: 'field', field: 'summary' })).toBe('summary');
    expect(currentRoute({ block_id: 'n', state: 'not_used' })).toBe(NOT_SENT);
    expect(currentRoute({ block_id: 'n', state: 'cannot_send', why: 'no' })).toBe(NOT_SENT);
    expect(currentRoute(undefined)).toBe(NOT_SENT);
  });
});

describe('routeOptions', () => {
  it('offers not sent, the prompt for a Prompt block only, then the fields of the destination', () => {
    const b = button();
    expect(routeOptions(b, blocks, blocks[0]!, fields, NOT_SENT).map((o) => o.value)).toEqual(['', PROMPT, 'summary', 'description', 'project']);
    expect(routeOptions(b, blocks, blocks[1]!, fields, NOT_SENT).map((o) => o.value)).toEqual(['', 'summary', 'description', 'project']);
  });

  it('shows a field another block fills, says which, and does not let it be taken twice', () => {
    const b = button({ mappings: [field('summary', 'note')] });
    const options = routeOptions(b, blocks, blocks[2]!, fields, NOT_SENT);
    const summary = options.find((o) => o.value === 'summary')!;
    expect(summary).toEqual({ value: 'summary', label: 'Summary (used by Note)', disabled: true });
    // the block that has it can keep it
    expect(routeOptions(b, blocks, blocks[1]!, fields, 'summary').find((o) => o.value === 'summary')!.disabled).toBe(false);
  });

  it('keeps a field typed by hand in the list, so the choice never lies', () => {
    const b = button({ mappings: [field('customfield_1', 'note')] });
    const options = routeOptions(b, blocks, blocks[1]!, fields, 'customfield_1');
    expect(options.map((o) => o.value)).toContain('customfield_1');
  });

  it('lists only not sent and the prompt when the destination offers no fields', () => {
    expect(routeOptions(button(), blocks, blocks[0]!, [], NOT_SENT).map((o) => o.value)).toEqual(['', PROMPT]);
  });
});

describe('applyRoute', () => {
  it('sends a block in a field of the destination', () => {
    const next = applyRoute(button(), 'note', 'description');
    expect(next.mappings).toEqual([field('description', 'note')]);
    expect(next.prompt_block_id).toBeNull();
  });

  it('moves a block to another field and leaves the rest of the mappings alone', () => {
    const rule = { field: 'build', source: { kind: 'rule' as const, value: 'build' } };
    const b = button({ mappings: [field('summary', 'proj'), field('description', 'note'), rule] });
    const next = applyRoute(b, 'note', 'project');
    expect(next.mappings).toEqual([field('summary', 'proj'), field('project', 'note'), rule]);
  });

  it('sends a block as the prompt, taking the prompt from the block that had it', () => {
    const b = button({ prompt_block_id: 'note', mappings: [field('summary', 'prompt')] });
    const next = applyRoute(b, 'prompt', PROMPT);
    expect(next.prompt_block_id).toBe('prompt');
    expect(next.mappings).toEqual([]); // it was in a field before: now it is the prompt only
    // and the old prompt block is simply not sent
    expect(applyRoute(b, 'note', NOT_SENT).prompt_block_id).toBeNull();
  });

  it('stops sending a block, whatever it was sent as', () => {
    const b = button({ prompt_block_id: 'prompt', mappings: [field('summary', 'note'), field('description', 'note')] });
    expect(applyRoute(b, 'note', NOT_SENT).mappings).toEqual([]);
    expect(applyRoute(b, 'prompt', NOT_SENT).prompt_block_id).toBeNull();
  });

  it('refuses to take a field another block fills', () => {
    const b = button({ mappings: [field('summary', 'note')] });
    const next = applyRoute(b, 'proj', 'summary');
    expect(next.mappings).toEqual([field('summary', 'note')]);
  });

  it('does not change the button it was given', () => {
    const b = button({ mappings: [field('summary', 'note')] });
    applyRoute(b, 'note', NOT_SENT);
    expect(b.mappings).toEqual([field('summary', 'note')]);
  });
});

describe('isAutomatic', () => {
  it('is true for the chat a Claude button opens when nothing maps it', () => {
    const r = { block_id: 'proj', state: 'sent' as const, role: 'field' as const, field: 'chat' };
    expect(isAutomatic(button(), 'claude', r)).toBe(true);
    expect(isAutomatic(button({ mappings: [field('chat', 'proj')] }), 'claude', r)).toBe(false);
    expect(isAutomatic(button(), 'jira', r)).toBe(false);
  });
});
