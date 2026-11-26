// The values of Data parsing as a person sees them: one value (`{map}`) read in one or more files.
// They are stored as before, file by file (a data rule per file, each with the values it reads), so
// settings exported by an older CargoFrame still load and the rules export as they always have.
// These functions turn the rules into values and back. A capture merges the steps of the same key:
// the newest file wins, or every different value (core/rules/src/run.rs).
import type { ItemEntry, Rule, RuleAcross, RuleValue } from '../../lib/types';
import { shortNameOk } from '../../lib/valueNames';
import { blankRule, slug, valuesOf } from '../ruleValues';
import { t } from '../../lib/i18n/index.svelte';

/** One place a value is read: a step of a rule. */
export interface Source {
  ruleId: string;
  rule: Rule;
  step: RuleValue;
}

export interface ValueEntry {
  key: string;
  name: string;
  sources: Source[];
  /** Made from other values, in no file. */
  combined: boolean;
  across: RuleAcross;
  /** What it is when no file has it; null: empty. */
  fallback: string | null;
}

type Rules = readonly ItemEntry<Rule>[];

/** Every value, in the order the rules list them. */
export function valueList(rules: Rules): ValueEntry[] {
  const byKey = new Map<string, ValueEntry>();
  for (const { id, item } of rules) {
    for (const step of valuesOf(id, item)) {
      let v = byKey.get(step.key);
      if (!v) {
        v = { key: step.key, name: '', sources: [], combined: true, across: 'newest', fallback: null };
        byKey.set(step.key, v);
      }
      if (!v.name && step.name.trim()) v.name = step.name.trim();
      v.sources.push({ ruleId: id, rule: item, step });
      if (step.find.type !== 'combine') v.combined = false;
      if (v.sources.length === 1) v.across = step.across ?? 'newest';
      if (v.fallback === null && step.fallback !== null) v.fallback = step.fallback;
    }
  }
  return [...byKey.values()].map((v) => ({ ...v, name: v.name || v.key }));
}

/** The files of a value, as written in its rules: "Game.log, Editor.log". */
export const filesOf = (v: ValueEntry) => [...new Set(v.sources.map((s) => s.rule.file.trim()).filter(Boolean))];

/** Why `name` cannot name a new value, or '' when it can. */
export function nameProblem(name: string, taken: readonly string[]): string {
  if (!name.trim()) return t('Give it a name.');
  const key = keyOf(name);
  if (!shortNameOk(key)) return t("{token} is kept for Jira's formatting, pick another name.", { token: `{${key}}` });
  if (taken.some((k) => k.toLowerCase() === key)) return t('{token} already exists.', { token: `{${key}}` });
  return '';
}

/** The key a name gives: "Map name" → `map_name`. */
export const keyOf = (name: string) => slug(name);

/** A rule's values as stored, a rule from before 0.3.0 turned into one value. */
function stepsOf(id: string, r: Rule): RuleValue[] {
  return valuesOf(id, r).map((v) => ({ ...v }));
}

/** The changes to save: rules by id (null: a new rule), and rules to delete. */
export interface Changes {
  save: { id: string | null; rule: Rule }[];
  remove: string[];
}

/** The rule that reads `file`, if one does (not one of values made from other values). */
function ruleFor(rules: Rules, file: string): ItemEntry<Rule> | undefined {
  const name = file.trim();
  return rules.find(({ item }) => item.file.trim() === name);
}

/** Read `step` in `file` too: in the rule of that file, or a new one. A step of the same key there is replaced. */
export function addSource(rules: Rules, file: string, step: RuleValue): Changes {
  const found = ruleFor(rules, file);
  if (found) {
    const values = stepsOf(found.id, found.item).filter((v) => v.key !== step.key);
    return { save: [{ id: found.id, rule: { ...found.item, values: [...values, step] } }], remove: [] };
  }
  const rule: Rule = { ...blankRule(), name: file.trim(), file: file.trim(), values: [step] };
  return { save: [{ id: null, rule }], remove: [] };
}

/** Change every step of `key` the same way: how several files are settled, what it is without a file. */
export function setAll(rules: Rules, key: string, patch: Partial<Pick<RuleValue, 'across' | 'fallback' | 'name'>>): Changes {
  const save: Changes['save'] = [];
  for (const { id, item } of rules) {
    const steps = stepsOf(id, item);
    if (!steps.some((s) => s.key === key)) continue;
    save.push({ id, rule: { ...item, values: steps.map((s) => (s.key === key ? { ...s, ...patch } : s)) } });
  }
  return { save, remove: [] };
}

/** Stop reading `key` in the rule `ruleId`; a rule left with nothing to do is deleted. */
export function removeSource(rules: Rules, ruleId: string, key: string): Changes {
  const found = rules.find((r) => r.id === ruleId);
  if (!found) return { save: [], remove: [] };
  const values = stepsOf(found.id, found.item).filter((v) => v.key !== key);
  const idle = values.length === 0 && !found.item.add_to_files && !found.item.log_time?.trim();
  if (idle) return { save: [], remove: [found.id] };
  return { save: [{ id: found.id, rule: { ...found.item, values, query: values.some((v) => v.find.type === 'advanced') ? found.item.query : '' } }], remove: [] };
}

/** Stop reading `key` anywhere. */
export function removeValue(rules: Rules, key: string): Changes {
  const out: Changes = { save: [], remove: [] };
  for (const r of rules) {
    if (!valuesOf(r.id, r.item).some((v) => v.key === key)) continue;
    const c = removeSource(rules, r.id, key);
    out.save.push(...c.save);
    out.remove.push(...c.remove);
  }
  return out;
}

/** Replace the step of `key` in rule `ruleId`. */
export function replaceStep(rules: Rules, ruleId: string, step: RuleValue): Changes {
  const found = rules.find((r) => r.id === ruleId);
  if (!found) return { save: [], remove: [] };
  const values = stepsOf(found.id, found.item).map((v) => (v.key === step.key ? step : v));
  return { save: [{ id: found.id, rule: { ...found.item, values } }], remove: [] };
}
