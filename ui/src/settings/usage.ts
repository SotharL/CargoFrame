/**
 * Where a value from a file ("data rule") or a destination field ("info type") is used, so the
 * pages that define them can say what each one is for instead of leaving it to guesswork.
 */
import type { ItemEntry, Layout } from '../lib/types';

type Layouts = readonly ItemEntry<Layout>[];

function names(layouts: Layouts, uses: (l: Layout) => boolean): string[] {
  return layouts.filter((l) => uses(l.item)).map((l) => l.item.name.trim() || l.id);
}

/** Layouts that put `{{rule.<id>}}` in a text block, or fill a field from the rule in a sharing button. */
export function layoutsUsingRule(layouts: Layouts, ruleId: string): string[] {
  const token = `{{rule.${ruleId}}}`;
  return names(layouts, (l) =>
    l.blocks.some(
      (b) =>
        (b.type === 'text' && b.default_text.includes(token)) ||
        (b.type === 'sharing_button' && b.mappings.some((m) => m.source.kind === 'rule' && m.source.value === ruleId)),
    ),
  );
}

/** Layouts with a sharing button that fills the destination field `fieldId`. */
export function layoutsUsingField(layouts: Layouts, fieldId: string): string[] {
  return names(layouts, (l) => l.blocks.some((b) => b.type === 'sharing_button' && b.mappings.some((m) => m.field === fieldId)));
}

/** "Used in Bug report, Daily note" or "Not used by any layout yet". */
export function usedIn(layoutNames: readonly string[]): string {
  if (layoutNames.length === 0) return 'Not used by any layout yet';
  const shown = layoutNames.slice(0, 3).join(', ');
  const more = layoutNames.length > 3 ? ` and ${layoutNames.length - 3} more` : '';
  return `Used in ${shown}${more}`;
}
