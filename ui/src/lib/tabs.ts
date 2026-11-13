// Tabs blocks, like a binder: each tab holds blocks laid out inside the Tabs block's area, and one
// tab shows at a time. The rules mirror core/layout (model.rs `tab_of`, `may_share`): the builder
// and the capture window both use them.
import type { Block } from './types';

export type TabsBlock = Extract<Block, { type: 'tabs' }>;

/** The fewest and the most tabs, and the smallest a Tabs block is each way (core/layout). */
export const MIN_TABS = 2;
export const MAX_TABS = 5;
export const MIN_TABS_SIZE = 2;

/** The tab open in each Tabs block, by its id; one not listed has its first tab open. */
export type OpenTabs = Readonly<Record<string, number>>;

/** The Tabs block a block is in, and the index of its tab. */
export function tabOf(blocks: readonly Block[], id: string): { tabs: TabsBlock; index: number } | null {
  for (const b of blocks) {
    if (b.type !== 'tabs') continue;
    const index = b.tabs.findIndex((t) => t.blocks?.includes(id));
    if (index >= 0) return { tabs: b, index };
  }
  return null;
}

/** The ids of the blocks in all the tabs of a Tabs block. */
export const childIds = (t: TabsBlock): string[] => t.tabs.flatMap((x) => x.blocks ?? []);

/** The tab open in `t`, within its tabs. */
export const openTab = (t: TabsBlock, open: OpenTabs): number => Math.min(Math.max(0, open[t.id] ?? 0), t.tabs.length - 1);

/** Is the block on screen: not in a tab, or in the open tab of its Tabs block? */
export function isShown(blocks: readonly Block[], id: string, open: OpenTabs): boolean {
  const t = tabOf(blocks, id);
  return !t || t.index === openTab(t.tabs, open);
}
