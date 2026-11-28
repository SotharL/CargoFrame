// What the service needs before a sharing button can send there, said beside the button, which
// stays grey until it has it:
// - the fields every share to it needs (Slack's channel, Reddit's subreddit, Jira's project to
//   create an issue), from the connector: a block that sends one and is still empty is named;
// - the fields it needs for what is picked, asked of the service: Jira's create screen for the
//   issue type in the project (`createmeta`, as `PROJ/10001`), a subreddit's post requirements
//   (as `rust`). A field no block sends, or sent by a block still empty, is named.
import type { Block, DropdownOption, FieldNeed, SessionView } from '../lib/types';
import type { FormValues } from './values';
import { isEmpty, nameOf } from './willSend';
import { t } from '../lib/i18n/index.svelte';

type SharingBlock = Extract<Block, { type: 'sharing_button' }>;

/** Always filled by the connector: Jira's summary and Reddit's title fall back to the window's title. */
const FILLED_ANYWAY = new Set(['summary', 'title']);

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
 * Does the button send something in `field`? A fixed value, a block that is not empty, or the
 * second step of one of its dropdowns (a Discord thread picked inside a channel).
 */
function sends(view: SessionView, button: SharingBlock, values: FormValues, field: string, stillTaken: boolean): boolean {
  const m = button.mappings.find((x) => x.field === field);
  if (m?.source.kind === 'fixed' && m.source.value.trim()) return true;
  if (m?.source.kind === 'block') {
    const b = blockOf(view, m.source.value);
    if (b && !isEmpty(view, b, values, stillTaken)) return true;
  }
  if (m?.source.kind === 'rule') return true; // read from files: not known before the share
  return button.mappings.some((x) => {
    if (x.source.kind !== 'block') return false;
    const b = blockOf(view, x.source.value);
    return b?.type === 'dropdown' && b.deeper?.field === field && (values.deeper[b.id]?.value ?? '').trim() !== '';
  });
}

/**
 * What to ask the service about, or null. Jira: `PROJ/10001`, unless an issue is picked to update
 * (its create screen does not apply) or no project or issue type is picked yet. Reddit: the
 * subreddit picked.
 */
export function needsAsk(view: SessionView, button: SharingBlock, values: FormValues): string | null {
  const kind = view.sharing[button.id]?.kind;
  if (kind === 'reddit') {
    const sub = valueOf(view, button, values, 'subreddit')?.replace(/^\/?r\//i, '');
    return sub && /^\w{2,21}$/.test(sub) ? sub : null;
  }
  if (kind !== 'jira') return null;
  if (valueOf(view, button, values, 'issue')) return null;
  const project = valueOf(view, button, values, 'project');
  const type = valueOf(view, button, values, 'issuetype');
  if (!project || !type || !/^\w+$/.test(project) || !/^\d+$/.test(type)) return null;
  return `${project}/${type}`;
}

/** The sentences for the fields the service asked for that the button does not send, in its order. */
export function unsent(
  view: SessionView,
  button: SharingBlock,
  values: FormValues,
  required: DropdownOption[],
  stillTaken = true,
): string[] {
  const jira = view.sharing[button.id]?.kind === 'jira';
  const out: string[] = [];
  for (const f of required) {
    if (FILLED_ANYWAY.has(f.value)) continue;
    const m = button.mappings.find((x) => x.field === f.value);
    if (!m) {
      out.push(
        jira
          ? t('Jira needs “{name}” for this issue type, and no block sends it', { name: f.label })
          : t('This subreddit requires “{name}”, and no block sends it', { name: f.label }),
      );
      continue;
    }
    if (m.source.kind !== 'block') continue;
    const b = blockOf(view, m.source.value);
    if (b && isEmpty(view, b, values, stillTaken)) {
      out.push(
        jira
          ? t('Jira needs “{name}”: fill “{block}”', { name: f.label, block: nameOf(b) })
          : t('This subreddit requires “{name}”: fill “{block}”', { name: f.label, block: nameOf(b) }),
      );
    }
  }
  return out;
}

/**
 * The sentences for the fields every share to the service needs that a block of the button sends
 * while still empty. A field no block sends is the layout's problem, already in the button's reasons.
 */
export function emptyNeeds(view: SessionView, button: SharingBlock, values: FormValues, stillTaken = true): string[] {
  const needs: FieldNeed[] = view.sharing[button.id]?.needs ?? [];
  const out: string[] = [];
  for (const n of needs) {
    if (sends(view, button, values, n.field, stillTaken)) continue;
    if (n.unless && sends(view, button, values, n.unless, stillTaken)) continue;
    const m = button.mappings.find((x) => x.field === n.field);
    const b = m?.source.kind === 'block' ? blockOf(view, m.source.value) : undefined;
    if (b) out.push(t('“{name}” is needed: fill “{block}”', { name: n.label, block: nameOf(b) }));
  }
  return out;
}
