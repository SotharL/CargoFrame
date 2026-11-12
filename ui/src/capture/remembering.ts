// What a window keeps of a share that went somewhere: the choices made in each dropdown, so the
// next capture starts on them. Only values and names are kept (see lib/recent).
import { deeperKey, remember } from '../lib/recent';
import type { Block, DropdownOption, SessionView } from '../lib/types';
import type { FormValues } from './values';

type Dropdown = Extract<Block, { type: 'dropdown' }>;

/** Lists a service sends start on the last choice used; chats keep their own order. */
export const remembers = (b: Dropdown) => b.remember !== false && b.source.type === 'web_api';

/**
 * After a share that went somewhere, the choices made become the recent ones. A post the share
 * made (`placed`: a forum post started through a webhook, which cannot be listed afterwards) is
 * kept as a choice inside its channel; so is a post picked from the list or by link.
 */
export function rememberChoices(view: SessionView, values: FormValues, placed: DropdownOption | null = null) {
  const layout = view.layout?.id;
  for (const b of view.layout?.blocks ?? []) {
    if (b.type === 'tags' && b.remember !== false) {
      // the first tag ends on top
      for (const tag of [...(values.tags?.[b.id] ?? [])].reverse()) remember(layout, b.id, { value: tag, label: tag });
      continue;
    }
    if (b.type !== 'dropdown' || !remembers(b)) continue;
    const value = values.dropdowns[b.id];
    const label = view.dropdowns[b.id]?.options.find((o) => o.value === value)?.label;
    if (value && label) remember(layout, b.id, { value, label });
    if (value && b.deeper) {
      const inside = placed ? { value: placed.value, label: placed.label } : values.deeper[b.id];
      if (inside?.value.trim()) remember(layout, deeperKey(b.id, value), { value: inside.value.trim(), label: inside.label || inside.value });
    }
  }
}
