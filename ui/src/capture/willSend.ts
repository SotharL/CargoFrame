import type { Block, BlockReach, ClipEdit, SessionView } from '../lib/types';
import type { FormValues } from './values';
import { formatSize } from './values';
import { initialTicks, isTicked, type Ticks } from './ticks';

type SharingBlock = Extract<Block, { type: 'sharing_button' }>;

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

function titleOf(b: Block): string {
  if (b.type === 'text' || b.type === 'dropdown' || b.type === 'files' || b.type === 'tags' || b.type === 'log_view') return b.title || b.id;
  return b.id;
}

/** A recording whose layout also has a screenshot block: that block starts empty. */
export function pairedWithRecording(view: SessionView): boolean {
  const blocks = view.layout?.blocks ?? [];
  return view.video != null && blocks.some((b) => b.type === 'video') && blocks.some((b) => b.type === 'screenshot');
}

function reachOf(view: SessionView, button: SharingBlock, blockId: string): BlockReach | undefined {
  return view.sharing[button.id]?.reach?.blocks.find((r) => r.block_id === blockId);
}

/**
 * The live line beside a sharing button: what this click will carry, in the order the person
 * thinks of it (picture, files, prompt, the rest). Only blocks whose box is ticked for this button
 * and that the destination can take appear; a block with nothing in it is left out, as the share
 * leaves it out. Without `ticks`, the layout's own choice (the boxes as the capture starts).
 */
export function willSend(
  view: SessionView,
  button: SharingBlock,
  values: FormValues,
  imageEdited: boolean,
  clipEdit: ClipEdit | null = null,
  ticks: Ticks = initialTicks(view),
  /** With a recording and a screenshot block: the block holds a picture (it starts empty). */
  stillTaken = true,
): string[] {
  const blocks = view.layout?.blocks ?? [];
  // A ticked block goes when the destination can take it. A ticked block the button had switched
  // off (picture, files) goes too: the box is how the person asks for it this time.
  const sent = (b: Block): BlockReach | undefined => {
    const r = reachOf(view, button, b.id);
    if (!r || !isTicked(ticks, button.id, b.id)) return undefined;
    return r.state === 'sent' || r.state === 'switched_off' ? r : undefined;
  };
  const parts: string[] = [];
  const kind = view.sharing[button.id]?.kind ?? null;

  for (const b of blocks) {
    const r = sent(b);
    if (b.type === 'screenshot' && r && (stillTaken || !pairedWithRecording(view))) {
      parts.push(imageEdited ? 'Screenshot (edited)' : 'Screenshot');
    }
    if (b.type === 'video' && r && view.video) {
      const whole = view.video.duration_ms / 1000;
      const from = (clipEdit?.start_ms ?? 0) / 1000;
      const to = (clipEdit?.end_ms ?? view.video.duration_ms) / 1000;
      const done = [from > 0 || to < whole ? 'cut' : '', clipEdit?.crop ? 'cropped' : '', clipEdit?.mark ? 'marked' : ''].filter(Boolean);
      if (done.length === 0) {
        parts.push(`Video (${whole.toFixed(1)} s, ${formatSize(view.video.bytes)})`);
      } else {
        const said = done.length > 1 ? `${done.slice(0, -1).join(', ')} and ${done[done.length - 1]}` : done[0];
        const length = done[0] === 'cut' ? `${(to - from).toFixed(1)} s of ${whole.toFixed(1)} s` : `${whole.toFixed(1)} s`;
        parts.push(`Video (${length}, ${said})`);
      }
    }
  }
  for (const b of blocks) {
    const r = sent(b);
    if (b.type !== 'files' || !r) continue;
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
