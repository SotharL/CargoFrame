// Choosing, for one sharing button, where each text or dropdown block goes: nowhere, the prompt, or
// one of the destination's own fields. The choice is a list of known fields, never typed text, so a
// misspelt field cannot happen; a field already taken by another block cannot be taken twice.
import type { Block, BlockReach, InfoMapping } from '../../lib/types';
import { blockName } from './reach';

type SharingBlock = Extract<Block, { type: 'sharing_button' }>;

/** The value of the "not sent" choice. */
export const NOT_SENT = '';
/** The value of the "prompt" choice (a field key never starts with an at sign). */
export const PROMPT = '@prompt';

export interface RouteField {
  key: string;
  label: string;
}

export interface RouteOption {
  value: string;
  label: string;
  disabled: boolean;
}

/** What the button does with the block now, as a choice. */
export function currentRoute(r: BlockReach | undefined): string {
  if (!r || r.state !== 'sent') return NOT_SENT;
  if (r.role === 'prompt') return PROMPT;
  if (r.role === 'field') return r.field;
  return NOT_SENT;
}

/** The mapping that feeds `field`, if a block does. */
const feeding = (button: SharingBlock, field: string) =>
  button.mappings.find((m) => m.field === field && m.source.kind === 'block');

/**
 * The choices for one block: not sent, the prompt (for a text block whose role is Prompt), then
 * every field the destination offers. A field another block already fills is shown but cannot be
 * taken, and says which block has it. A field the block already has but the destination does not
 * list (one the person typed by hand in the field list below) stays as it is.
 */
export function routeOptions(
  button: SharingBlock,
  blocks: Block[],
  block: Block,
  fields: RouteField[],
  current: string,
): RouteOption[] {
  const out: RouteOption[] = [{ value: NOT_SENT, label: 'Not sent', disabled: false }];
  if (block.type === 'text' && block.role === 'prompt') out.push({ value: PROMPT, label: 'The prompt', disabled: false });
  const known = new Set(fields.map((f) => f.key));
  const own = current !== NOT_SENT && current !== PROMPT && !known.has(current) ? [{ key: current, label: current }] : [];
  for (const f of [...fields, ...own]) {
    const taker = feeding(button, f.key);
    const other = taker && taker.source.value !== block.id ? blocks.find((b) => b.id === taker.source.value) : undefined;
    out.push({
      value: f.key,
      label: other ? `${f.label} (used by ${blockName(other)})` : f.label,
      disabled: other !== undefined,
    });
  }
  return out;
}

/**
 * The button's prompt and mappings after `blockId` is sent as `route`. Everything else is left as
 * it was: other blocks, mappings that come from a data rule or a fixed value.
 */
export function applyRoute(
  button: SharingBlock,
  blockId: string,
  route: string,
): { prompt_block_id: string | null; mappings: InfoMapping[] } {
  const fromThis = (m: InfoMapping) => m.source.kind === 'block' && m.source.value === blockId;
  let prompt = button.prompt_block_id;
  if (prompt === blockId) prompt = null;
  let mappings = button.mappings.filter((m) => !fromThis(m));
  if (route === PROMPT) {
    prompt = blockId;
  } else if (route !== NOT_SENT) {
    // a field is filled by one block: taking it from another block is refused, not done silently
    if (!mappings.some((m) => m.field === route && m.source.kind === 'block')) {
      const at = button.mappings.findIndex(fromThis);
      const mapping: InfoMapping = { field: route, source: { kind: 'block', value: blockId } };
      mappings = at < 0 ? [...mappings, mapping] : [...mappings.slice(0, at), mapping, ...mappings.slice(at)];
    }
  }
  return { prompt_block_id: prompt, mappings };
}

/** The destination fills this on its own (Claude's chat to open): it cannot be switched off here. */
export function isAutomatic(button: SharingBlock, kind: string | null, r: BlockReach | undefined): boolean {
  return kind === 'claude' && r?.state === 'sent' && r.role === 'field' && r.field === 'chat'
    && !feeding(button, 'chat');
}
