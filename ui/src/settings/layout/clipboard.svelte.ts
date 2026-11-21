// The block copied in the layout builder (with the blocks of its tabs), kept while the Settings
// window is open: it can be pasted in the same layout or in another one.
import type { Block } from '../../lib/types';

let copied = $state<Block[] | null>(null);

export const clipboard = {
  get blocks(): readonly Block[] | null {
    return copied;
  },
  set(blocks: Block[]) {
    copied = blocks.length ? blocks : null;
  },
  clear() {
    copied = null;
  },
};
