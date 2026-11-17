import type { Block, BlockReach, ClipEdit, SessionView } from '../lib/types';
import type { FormValues } from './values';
import { formatSize } from './values';
import { initialTicks, isTicked, type Ticks } from './ticks';
import { t, tn } from '../lib/i18n/index.svelte';

type SharingBlock = Extract<Block, { type: 'sharing_button' }>;

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
      parts.push(imageEdited ? t('Screenshot (edited)') : t('Screenshot'));
    }
    if (b.type === 'video' && r && view.video) {
      const whole = view.video.duration_ms / 1000;
      const from = (clipEdit?.start_ms ?? 0) / 1000;
      const to = (clipEdit?.end_ms ?? view.video.duration_ms) / 1000;
      const cut = from > 0 || to < whole;
      const done = [cut ? t('cut') : '', clipEdit?.crop ? t('cropped') : '', clipEdit?.mark ? t('marked') : '', clipEdit?.keys ? t('with the keys') : '', clipEdit?.clock ? t('with the time') : ''].filter(Boolean);
      if (done.length === 0) {
        parts.push(t('Video ({seconds} s, {size})', { seconds: whole.toFixed(1), size: formatSize(view.video.bytes) }));
      } else {
        const said = done.length > 1 ? t('{list} and {last}', { list: done.slice(0, -1).join(', '), last: done[done.length - 1]! }) : done[0]!;
        const length = cut
          ? t('{seconds} s of {whole} s', { seconds: (to - from).toFixed(1), whole: whole.toFixed(1) })
          : t('{seconds} s', { seconds: whole.toFixed(1) });
        parts.push(t('Video ({length}, {what})', { length, what: said }));
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
    parts.push(
      asText
        ? tn('{n} file ({size}), text ones as text', '{n} files ({size}), text ones as text', entries.length, { size: formatSize(size) })
        : tn('{n} file ({size})', '{n} files ({size})', entries.length, { size: formatSize(size) }),
    );
  }
  for (const b of blocks) {
    const r = sent(b);
    if (b.type === 'text' && r?.state === 'sent' && r.role === 'prompt') parts.push(t('Prompt'));
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
  if (!chat) return t('a new chat');
  const value = values.dropdowns[chat.id];
  const option = view.dropdowns[chat.id]?.options.find((o) => o.value === value);
  if (!value || value === 'new') return t('a new chat');
  return option?.label ?? t('the chat you picked');
}
