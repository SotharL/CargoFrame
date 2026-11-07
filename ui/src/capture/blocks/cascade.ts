// How a long list is narrowed step by step: server, then category, then the choice itself. This is
// only the logic; DropdownBlock draws it. A list whose options carry no `server` is a plain list.
import type { ComboOption, DropdownOption } from '../../lib/types';
import type { Recent } from '../../lib/recent';

/** The "choice outside any category" entry of the category list. */
export const NO_CATEGORY = '\u0000none';
/** The "any category" entry: nothing is narrowed. */
export const ALL = '';

const unique = (xs: (string | null | undefined)[]): string[] => [...new Set(xs.filter((x): x is string => !!x))];

export const hasLevels = (options: readonly DropdownOption[]) => options.some((o) => !!o.server);

export const serversOf = (options: readonly DropdownOption[]) => unique(options.map((o) => o.server));

/** The categories inside one server, in the order the service lists them. */
export function categoriesIn(options: readonly DropdownOption[], server: string | null): string[] {
  return unique(options.filter((o) => o.server === server).map((o) => o.category));
}

/**
 * Whether a category step is worth showing: two or more places to choose between. A server with
 * one category (or none) goes straight to its choices.
 */
export function needsCategoryStep(options: readonly DropdownOption[], server: string | null): boolean {
  const inServer = options.filter((o) => o.server === server);
  const places = categoriesIn(options, server).length + (inServer.some((o) => !o.category) ? 1 : 0);
  return places >= 2;
}

/** The options of one server, and of one category inside it when `category` is not ALL. */
export function scope(options: readonly DropdownOption[], server: string | null, category: string): DropdownOption[] {
  if (!hasLevels(options)) return [...options];
  return options.filter((o) => {
    if (o.server !== server) return false;
    if (category === ALL) return true;
    return category === NO_CATEGORY ? !o.category : o.category === category;
  });
}

/** The entry to start on: the most recently used one that still exists, else the first listed. */
export function startOn(options: readonly DropdownOption[], recent: readonly Recent[]): DropdownOption | undefined {
  for (const r of recent) {
    const found = options.find((o) => o.value === r.value);
    if (found) return found;
  }
  return options[0];
}

/**
 * Narrow a list to the choices whose name contains `text` (a thread named like the title).
 * Nothing matching is not an empty list: everything is shown, and `matched` says so.
 */
export function narrow(
  options: readonly DropdownOption[],
  text: string,
): { options: DropdownOption[]; applied: boolean; matched: boolean } {
  const t = text.trim().toLowerCase();
  if (!t) return { options: [...options], applied: false, matched: true };
  const found = options.filter((o) => o.label.toLowerCase().includes(t));
  return found.length > 0
    ? { options: found, applied: true, matched: true }
    : { options: [...options], applied: false, matched: false };
}

/** Where a choice is, for the small note after its name. */
export const placeOf = (o: DropdownOption) => [o.server, o.category].filter(Boolean).join(' · ');

/** The choices recently used that are still in the list, with their current names. */
export function recentIn(options: readonly DropdownOption[], recent: readonly Recent[]): DropdownOption[] {
  return recent.flatMap((r) => options.filter((o) => o.value === r.value));
}

/**
 * What the combobox shows: recent choices first under their own heading, then the rest. A recent
 * choice says where it is (it may be in another server); the others say their category.
 */
export function comboOptions(
  options: readonly DropdownOption[],
  recent: readonly DropdownOption[],
  withPlace: boolean,
): ComboOption[] {
  const row = (o: DropdownOption, group: string | null, note: string | null): ComboOption => ({
    value: o.value, label: o.label, note: withPlace ? note : null, group,
  });
  const plain = (o: DropdownOption, group: string | null) => row(o, group, o.category ?? null);
  if (recent.length === 0) return options.map((o) => plain(o, null));
  const seen = new Set(recent.map((o) => o.value));
  return [
    ...recent.map((o) => row(o, 'Recent', placeOf(o) || null)),
    ...options.filter((o) => !seen.has(o.value)).map((o) => plain(o, 'Others')),
  ];
}
