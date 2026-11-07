// When a sharing button has nowhere to post. A dropdown that asks the service for its places (the
// servers a Discord bot is in) and gets none back leaves the button with no destination: it then
// keeps a zip on this computer instead, and says so, the same as a button with no connection.
import type { Block, SessionView } from '../lib/types';
import type { FormValues } from './values';

type SharingBlock = Extract<Block, { type: 'sharing_button' }>;

/** True when a list this button fills a field from came back empty, so there is nowhere to send. */
export function nowhereToPost(view: SessionView, button: SharingBlock, _values?: FormValues): boolean {
  return button.mappings.some((m) => {
    if (m.source.kind !== 'block') return false;
    const block = view.layout?.blocks.find((b) => b.id === m.source.value);
    if (!block || block.type !== 'dropdown' || block.source.type !== 'web_api' || block.optional) return false;
    const state = view.dropdowns[block.id];
    // Only a list that loaded and was empty: a failed load is a problem to show, not to hide.
    return state?.status === 'ready' && state.options.length === 0;
  });
}
