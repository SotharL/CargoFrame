import type { Block, ClipEdit, SessionView, ShareRequest } from '../lib/types';
import type { FormValues } from './values';
import type { Ticks } from './ticks';
import { nowhereToPost } from './destination';

export type { ShareRequest };

type SharingBlock = Extract<Block, { type: 'sharing_button' }>;

/**
 * Gather what one sharing button should send. Only blocks that are in the layout are included,
 * so a stale value from a replaced capture can never leak into a share. `ticks` are the boxes of
 * the window: the blocks this button was asked to send. Without them the layout's own choice goes.
 */
export function collectShare(
  view: SessionView,
  button: SharingBlock,
  values: FormValues,
  imageEdited: boolean,
  clipEdit: ClipEdit | null = null,
  ticks: Ticks | null = null,
  still = false,
): ShareRequest {
  const ids = new Set(view.layout?.blocks.map((b) => b.id) ?? []);
  const keep = <T>(m: Record<string, T>): Record<string, T> =>
    Object.fromEntries(Object.entries(m).filter(([id]) => ids.has(id)));
  const texts = keep(values.texts);
  const sendBlocks = ticks?.[button.id]?.filter((id) => ids.has(id)) ?? null;
  // A prompt whose box was cleared is not sent, so it does not have to be written.
  const prompt = button.prompt_block_id && (sendBlocks === null || sendBlocks.includes(button.prompt_block_id))
    ? (texts[button.prompt_block_id] ?? null)
    : null;
  return {
    session_id: view.session_id,
    trigger_id: view.trigger_id,
    button_id: button.id,
    connection_id: button.connection_id,
    prompt,
    texts,
    dropdowns: keep(values.dropdowns),
    files: keep(values.files),
    image_edited: imageEdited,
    still,
    clip_edit: view.video ? clipEdit : null,
    send_blocks: sendBlocks,
    save_here: nowhereToPost(view, button, values) || undefined,
  };
}

/**
 * A bound prompt must have text before sharing to an AI chat. The backend decides whether the
 * button is allowed at all; this only checks what the person typed since the capture.
 */
export function promptIsEmpty(request: ShareRequest): boolean {
  return request.prompt !== null && request.prompt.trim() === '';
}
