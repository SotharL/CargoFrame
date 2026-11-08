// Data from files: the values a rule reads, their variable names, and rules made before 0.3.0.
import type { Rule, RuleFind, RuleValue } from '../lib/types';

/** A variable name from what a person calls the value: "OS Name" → `os_name`. */
export function slug(name: string): string {
  const s = name
    .normalize('NFKD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '')
    .slice(0, 48);
  return s || 'value';
}

/** `wanted`, or `wanted_2`, `wanted_3`… so it is not one of `taken`. */
export function unique(wanted: string, taken: Iterable<string>): string {
  const used = new Set(taken);
  if (!used.has(wanted)) return wanted;
  for (let n = 2; ; n++) {
    const k = `${wanted}_${n}`;
    if (!used.has(k)) return k;
  }
}

/** The values of a rule; a rule from before 0.3.0 reads one, named by the rule's id. */
export function valuesOf(id: string, r: Rule): RuleValue[] {
  if (r.values && r.values.length > 0) return r.values;
  if (!r.query?.trim()) return [];
  return [{ key: id, name: r.name, find: { type: 'advanced' }, which: 'first', fallback: r.fallback ?? null }];
}

/** Every variable name the other rules use. */
export function keysOfOthers(rules: { id: string; item: Rule }[], except: string | null): string[] {
  return rules.filter((r) => r.id !== except).flatMap((r) => valuesOf(r.id, r.item).map((v) => v.key));
}

/** A rule with nothing in it. */
export function blankRule(): Rule {
  return {
    name: '', file: '', several: 'newest', add_to_files: false, values: [],
    format: 'json', query: '', multiple: 'first', separator: ', ', nth: 1, output: '', fallback: null, max_bytes: 1048576,
  };
}

/** A new value, named so its variable does not clash. */
export function newValue(taken: string[]): RuleValue {
  return { key: unique('value', taken), name: '', find: { type: 'value_of', name: '' }, which: 'first', fallback: null };
}

/** The sentence a card shows for one value: "after “GPU:”". */
export function describe(find: RuleFind, r: Rule): string {
  switch (find.type) {
    case 'value_of':
      return `after “${find.name}”`;
    case 'between':
      return find.end ? `between “${find.start}” and “${find.end}”` : `after “${find.start}”, to the end of the line`;
    case 'line_with':
      return `the line with “${find.text}”`;
    case 'advanced':
      return `${r.format.toUpperCase()}: ${r.query ?? ''}`;
  }
}

/** The variables of a rule, for "where is it used". */
export function tokens(id: string, r: Rule): string[] {
  return valuesOf(id, r).map((v) => `{{rule.${v.key}}}`);
}
