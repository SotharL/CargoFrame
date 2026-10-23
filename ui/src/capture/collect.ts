import type { Block, SessionView } from '../lib/types';
import type { FormValues } from './values';

/** Everything the sharing button sends, apart from the picture itself. */
export interface ShareRequest {
  session_id: number;
  trigger_id: string;
  button_id: string;
  /** The sharing button's destination connection. */
  connection_id: string | null;
  /** The Prompt text block bound to the button, with what the person typed. */
  prompt: string | null;
  texts: Record<string, string>;
  dropdowns: Record<string, string | null>;
  files: Record<string, string[]>;
  /** True when the person changed the picture (the sent PNG is then the flattened one). */
  image_edited: boolean;
}

type SharingBlock = Extract<Block, { type: 'sharing_button' }>;

/**
 * Gather what one sharing button should send. Only blocks that are in the layout are included,
 * so a stale value from a replaced capture can never leak into a share.
 */
export function collectShare(
  view: SessionView,
  button: SharingBlock,
  values: FormValues,
  imageEdited: boolean,
): ShareRequest {
  const ids = new Set(view.layout?.blocks.map((b) => b.id) ?? []);
  const keep = <T>(m: Record<string, T>): Record<string, T> =>
    Object.fromEntries(Object.entries(m).filter(([id]) => ids.has(id)));
  const texts = keep(values.texts);
  return {
    session_id: view.session_id,
    trigger_id: view.trigger_id,
    button_id: button.id,
    connection_id: button.connection_id,
    prompt: button.prompt_block_id ? (texts[button.prompt_block_id] ?? null) : null,
    texts,
    dropdowns: keep(values.dropdowns),
    files: keep(values.files),
    image_edited: imageEdited,
  };
}

/**
 * A bound prompt must have text before sharing to an AI chat. The backend decides whether the
 * button is allowed at all; this only checks what the person typed since the capture.
 */
export function promptIsEmpty(request: ShareRequest): boolean {
  return request.prompt !== null && request.prompt.trim() === '';
}
