import type { Block, BlockReach, ClipEdit, ServiceKind, SessionView } from '../lib/types';
import type { FormValues } from './values';
import { formatSize } from './values';

type SharingBlock = Extract<Block, { type: 'sharing_button' }>;

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

function titleOf(b: Block): string {
  if (b.type === 'text' || b.type === 'dropdown' || b.type === 'files') return b.title || b.id;
  return b.id;
}

function reachOf(view: SessionView, button: SharingBlock, blockId: string): BlockReach | undefined {
  return view.sharing[button.id]?.reach?.blocks.find((r) => r.block_id === blockId);
}

/**
 * The live line beside a sharing button: what this click will carry, in the order the person
 * thinks of it (picture, files, prompt, the rest). Only blocks the destination can take and the
 * button is set to send appear; a block with nothing in it is left out, as the share leaves it out.
 */
export function willSend(
  view: SessionView,
  button: SharingBlock,
  values: FormValues,
  imageEdited: boolean,
  clipEdit: ClipEdit | null = null,
): string[] {
  const blocks = view.layout?.blocks ?? [];
  const sent = (b: Block) => reachOf(view, button, b.id);
  const parts: string[] = [];
  const kind = view.sharing[button.id]?.kind ?? null;

  for (const b of blocks) {
    const r = sent(b);
    if (b.type === 'screenshot' && r?.state === 'sent') {
      parts.push(imageEdited ? 'Screenshot (edited)' : 'Screenshot');
    }
    if (b.type === 'video' && r?.state === 'sent' && view.video) {
      const whole = view.video.duration_ms / 1000;
      const cropped = clipEdit?.crop ? 'cropped' : '';
      const from = (clipEdit?.start_ms ?? 0) / 1000;
      const to = (clipEdit?.end_ms ?? view.video.duration_ms) / 1000;
      const cut = from > 0 || to < whole ? 'cut' : '';
      if (clipEdit && (cut || cropped)) {
        const said = [cut, cropped].filter(Boolean).join(' and ');
        parts.push(cut ? `Video (${(to - from).toFixed(1)} s of ${whole.toFixed(1)} s, ${said})` : `Video (${whole.toFixed(1)} s, ${said})`);
      } else {
        parts.push(`Video (${whole.toFixed(1)} s, ${formatSize(view.video.bytes)})`);
      }
    }
  }
  for (const b of blocks) {
    const r = sent(b);
    if (b.type !== 'files' || r?.state !== 'sent') continue;
    const ticked = new Set(values.files[b.id] ?? []);
    const entries = (view.files[b.id]?.entries ?? []).filter((e) => ticked.has(e.path));
    if (entries.length === 0) continue;
    const size = entries.reduce((sum, e) => sum + e.size, 0);
    const asText = kind === 'claude' && button.send?.files === 'inline';
    parts.push(`${plural(entries.length, 'file', 'files')} (${formatSize(size)})${asText ? ', text ones as text' : ''}`);
  }
  for (const b of blocks) {
    const r = sent(b);
    if (b.type === 'text' && r?.state === 'sent' && r.role === 'prompt') parts.push('Prompt');
  }
  for (const b of blocks) {
    const r = sent(b);
    if (r?.state !== 'sent' || r.role !== 'field') continue;
    // The chat to open is said on its own line; it is not content.
    if (kind === 'claude' && r.field === 'chat') continue;
    if (b.type === 'text' && (values.texts[b.id] ?? '').trim() !== '') parts.push(titleOf(b));
    if (b.type === 'dropdown' && values.dropdowns[b.id]) parts.push(titleOf(b));
  }
  return parts;
}

/** The name of the Claude chat the button will open, or `null` for any other destination. */
export function chatToOpen(view: SessionView, button: SharingBlock, values: FormValues): string | null {
  if (view.sharing[button.id]?.kind !== 'claude') return null;
  const blocks = view.layout?.blocks ?? [];
  const chat = blocks.find((b) => {
    const r = reachOf(view, button, b.id);
    return b.type === 'dropdown' && r?.state === 'sent' && r.role === 'field' && r.field === 'chat';
  });
  if (!chat) return 'a new chat';
  const value = values.dropdowns[chat.id];
  const option = view.dropdowns[chat.id]?.options.find((o) => o.value === value);
  if (!value || value === 'new') return 'a new chat';
  return option?.label ?? 'the chat you picked';
}

export type MarkState = 'sent' | 'cannot_send' | 'switched_off' | 'not_used';

export interface Mark {
  buttonId: string;
  label: string;
  kind: ServiceKind | null;
  state: MarkState;
  /** Why it cannot be sent, when it cannot. */
  why: string | null;
}

/** How one block relates to every sharing button of the layout. */
export interface BlockMarks {
  /** One entry per button that sends the block or cannot send it. */
  marks: Mark[];
  /** No button sends it: one plain "Not sent" says so instead of a chip per button. */
  notSent: boolean;
  /** Why not, for people who hover it: left out, not set up, or cannot be sent. */
  notSentWhy: string | null;
}

export function marksFor(view: SessionView, blockId: string): BlockMarks {
  const buttons = (view.layout?.blocks ?? []).filter((b): b is SharingBlock => b.type === 'sharing_button');
  const marks: Mark[] = [];
  const reasons: string[] = [];
  let anySent = false;
  for (const button of buttons) {
    const sharing = view.sharing[button.id];
    const r = sharing?.reach?.blocks.find((x) => x.block_id === blockId);
    if (!r) continue;
    const label = sharing?.mode === 'save_locally' ? 'Save locally' : button.label || 'Share';
    if (r.state === 'sent') {
      anySent = true;
      marks.push({ buttonId: button.id, label, kind: sharing?.kind ?? null, state: 'sent', why: null });
    } else if (r.state === 'cannot_send') {
      marks.push({ buttonId: button.id, label, kind: sharing?.kind ?? null, state: 'cannot_send', why: r.why });
      reasons.push(`${label} cannot send it: ${r.why}`);
    } else if (r.state === 'switched_off') {
      reasons.push(`${label} is set to leave it out`);
    } else {
      reasons.push(`${label} has no field for it`);
    }
  }
  const hasReach = buttons.some((b) => view.sharing[b.id]?.reach);
  return {
    marks,
    notSent: hasReach && !anySent,
    notSentWhy: hasReach && !anySent ? reasons.join('; ') || null : null,
  };
}
