// What the service needs before a sharing button can create something there. For Jira: the
// fields its create screen requires for the issue type picked in the project picked (Jira's
// `createmeta`, asked through the button as `PROJ/10001`). A field no block sends, or sent by a
// block still empty, is said beside the button, which stays grey: Jira would refuse the ticket.
import type { Block, DropdownOption, SessionView } from '../lib/types';
import type { FormValues } from './values';
import { isEmpty, nameOf } from './willSend';
import { t } from '../lib/i18n/index.svelte';

type SharingBlock = Extract<Block, { type: 'sharing_button' }>;

/** Always filled by the connector: the summary falls back to the window's title. */
const FILLED_ANYWAY = new Set(['summary']);

function blockOf(view: SessionView, id: string): Block | undefined {
  return view.layout?.blocks.find((b) => b.id === id);
}

/** The value a button sends in `field`: a choice or a text of its block, or a fixed value. */
function valueOf(view: SessionView, button: SharingBlock, values: FormValues, field: string): string | null {
  const m = button.mappings.find((x) => x.field === field);
  if (!m) return null;
  if (m.source.kind === 'fixed') return m.source.value.trim() || null;
  if (m.source.kind !== 'block') return null;
  const b = blockOf(view, m.source.value);
  if (b?.type === 'dropdown') return (values.dropdowns[b.id] ?? '').trim() || null;
  if (b?.type === 'text') return (values.texts[b.id] ?? '').trim() || null;
  return null;
}

/**
 * What to ask Jira about, `PROJ/10001`, or null: not a Jira button, an issue picked to update
 * (Jira's create screen does not apply), or no project or issue type picked yet.
 */
export function needsAsk(view: SessionView, button: SharingBlock, values: FormValues): string | null {
  if (view.sharing[button.id]?.kind !== 'jira') return null;
  if (valueOf(view, button, values, 'issue')) return null;
  const project = valueOf(view, button, values, 'project');
  const kind = valueOf(view, button, values, 'issuetype');
  if (!project || !kind || !/^\w+$/.test(project) || !/^\d+$/.test(kind)) return null;
  return `${project}/${kind}`;
}

/** The sentences for the required fields the button does not send, in Jira's order. */
export function unsent(
  view: SessionView,
  button: SharingBlock,
  values: FormValues,
  required: DropdownOption[],
  stillTaken = true,
): string[] {
  const out: string[] = [];
  for (const f of required) {
    if (FILLED_ANYWAY.has(f.value)) continue;
    const m = button.mappings.find((x) => x.field === f.value);
    if (!m) {
      out.push(t('Jira needs “{name}” for this issue type, and no block sends it', { name: f.label }));
      continue;
    }
    if (m.source.kind !== 'block') continue;
    const b = blockOf(view, m.source.value);
    if (b && isEmpty(view, b, values, stillTaken)) {
      out.push(t('Jira needs “{name}”: fill “{block}”', { name: f.label, block: nameOf(b) }));
    }
  }
  return out;
}
