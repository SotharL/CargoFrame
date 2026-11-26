// For a sharing button of a connection that can update an issue (Jira, GitHub): what each mapped
// block does to the issue picked (replace it, add below, add one line, add to the list, count one
// more, leave it), and which blocks must be filled before the button shares. The choices are
// lists, never typed text, and only those that make sense for the block and the field are shown.
import type { Block, ServiceKind, UpdateMode } from '../../lib/types';
import { msg, t } from '../../lib/i18n/index.svelte';

type SharingBlock = Extract<Block, { type: 'sharing_button' }>;

/** The connections whose share can update an existing issue. */
const UPDATES: readonly ServiceKind[] = ['jira', 'github'];

export const canUpdate = (kind: ServiceKind | null): boolean => kind !== null && UPDATES.includes(kind);

/** Fields that pick what is updated, or say how: not themselves updated. */
const PICKERS = new Set(['issue', 'comment', 'project', 'issuetype', 'repo']);
/** Fields that are lists: adding keeps what is there. */
const LISTS = new Set(['labels', 'components', 'fixVersions', 'versions', 'assignees']);
/** Fields of one line: "add below" joins on the same line. */
const TEXTS = new Set(['summary', 'description', 'title', 'body', 'environment']);

const LABEL: Record<UpdateMode, string> = {
  replace: msg('Replace it'),
  append: msg('Add below what is there'),
  append_line: msg('Add the first line below'),
  add: msg('Add to the list'),
  increment: msg('Count up by the value (1 when empty)'),
  skip: msg('Leave it as it is'),
};

export interface UpdateOption {
  value: UpdateMode;
  label: string;
}

/** The choices for the block that fills `field`; none when the field is not one an update changes. */
export function updateOptions(block: Block, field: string): UpdateOption[] {
  if (PICKERS.has(field)) return [];
  const modes: UpdateMode[] = ['replace'];
  if (LISTS.has(field) && (block.type === 'tags' || block.type === 'text' || block.type === 'dropdown')) modes.push('add');
  if (block.type === 'text') {
    modes.push('append', 'append_line');
    // a custom field can be a number; a known text field cannot
    if (!TEXTS.has(field) && !LISTS.has(field)) modes.push('increment');
  }
  modes.push('skip');
  return modes.map((value) => ({ value, label: t(LABEL[value]) }));
}

/** What the mapping of `field` does now (none set: replace). */
export function updateOf(button: SharingBlock, field: string): UpdateMode {
  return button.mappings.find((m) => m.field === field)?.update ?? 'replace';
}

/** The mappings after the block that fills `field` is set to do `mode` (replace is left unsaid). */
export function setUpdate(button: SharingBlock, field: string, mode: UpdateMode): SharingBlock['mappings'] {
  return button.mappings.map((m) => {
    if (m.field !== field) return m;
    const { update: _, ...rest } = m;
    return mode === 'replace' ? rest : { ...rest, update: mode };
  });
}

/** Blocks a share can wait for: something to write, pick, tick, or the capture itself. */
export function fillable(blocks: readonly Block[]): Block[] {
  return blocks.filter((b) => ['text', 'dropdown', 'tags', 'files', 'screenshot', 'video'].includes(b.type));
}

/** The blocks that must be filled after `id` is ticked (`on`) or not; ids of blocks gone are dropped. */
export function setRequired(button: SharingBlock, blocks: readonly Block[], id: string, on: boolean): string[] {
  const now = new Set((button.required ?? []).filter((r) => blocks.some((b) => b.id === r)));
  if (on) now.add(id);
  else now.delete(id);
  // in the order of the window, so the list of what is missing reads top to bottom
  return blocks.filter((b) => now.has(b.id)).map((b) => b.id);
}
