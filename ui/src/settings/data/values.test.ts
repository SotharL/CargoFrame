import { describe, expect, it } from 'vitest';
import type { ItemEntry, Rule, RuleValue } from '../../lib/types';
import { blankRule } from '../ruleValues';
import { addSource, nameProblem, removeSource, removeValue, setAll, valueList } from './values';

const pick = (key: string, lines_with: string): RuleValue => ({
  key, name: key[0]!.toUpperCase() + key.slice(1), find: { type: 'pick', lines_with, stop: '?' }, which: 'last', fallback: null,
});
const rule = (file: string, values: RuleValue[], more: Partial<Rule> = {}): Rule => ({ ...blankRule(), name: file, file, values, ...more });
const RULES: ItemEntry<Rule>[] = [
  { id: 'game', item: rule('Game.log', [pick('map', 'Maps/'), pick('build', 'Build:')], { add_to_files: true }) },
  { id: 'editor', item: rule('Editor.log', [{ ...pick('map', 'Maps/'), across: 'all', fallback: 'none' }]) },
  // made before 0.3.0: one value, named by the rule
  { id: 'version', item: { ...blankRule(), name: 'Version', file: 'package.json', format: 'json', query: 'version' } },
];

describe('the values of Data & fields', () => {
  it('are the rules’ values by key, read in one or more files', () => {
    const list = valueList(RULES);
    expect(list.map((v) => v.key)).toEqual(['map', 'build', 'version']);
    const map = list[0]!;
    expect(map.sources.map((s) => s.ruleId)).toEqual(['game', 'editor']);
    expect(map.name).toBe('Map');
    // the first step says how several files are settled; the first fallback is the one used
    expect(map.across).toBe('newest');
    expect(map.fallback).toBe('none');
    expect(list[2]!.sources[0]!.step.find.type).toBe('advanced');
  });

  it('are read in another file in that file’s rule, or a new one', () => {
    const more = addSource(RULES, 'Editor.log', pick('build', 'Build:'));
    expect(more.save).toHaveLength(1);
    expect(more.save[0]!.id).toBe('editor');
    expect(more.save[0]!.rule.values!.map((v) => v.key)).toEqual(['map', 'build']);
    // the same key in that file is replaced, not added twice
    const again = addSource(RULES, 'Game.log', { ...pick('map', 'LoadMap('), which: 'first' });
    expect(again.save[0]!.rule.values!.filter((v) => v.key === 'map')).toHaveLength(1);
    expect(again.save[0]!.rule.values!.find((v) => v.key === 'map')!.which).toBe('first');
    const fresh = addSource(RULES, 'Crash.log', pick('map', 'Maps/'));
    expect(fresh.save[0]).toMatchObject({ id: null, rule: { file: 'Crash.log', name: 'Crash.log', add_to_files: false } });
  });

  it('settle several files the same way everywhere, and stop being read where asked', () => {
    const all = setAll(RULES, 'map', { across: 'all', fallback: null });
    expect(all.save.map((s) => s.id)).toEqual(['game', 'editor']);
    expect(all.save.flatMap((s) => s.rule.values!.filter((v) => v.key === 'map').map((v) => [v.across, v.fallback]))).toEqual([
      ['all', null],
      ['all', null],
    ]);
    // a rule left with nothing to do goes; one that adds its file stays
    expect(removeSource(RULES, 'editor', 'map')).toEqual({ save: [], remove: ['editor'] });
    expect(removeSource(RULES, 'game', 'map').save[0]!.rule.values!.map((v) => v.key)).toEqual(['build']);
    expect(removeValue(RULES, 'map').remove).toEqual(['editor']);
    expect(removeValue(RULES, 'version').remove).toEqual(['version']);
  });

  it('need a name no other value has, whatever its case, and not one Jira keeps', () => {
    const taken = ['map', 'Build'];
    expect(nameProblem('Map', taken)).toBe('{map} already exists.');
    expect(nameProblem('builds', taken)).toBe('');
    expect(nameProblem('build', taken)).toBe('{build} already exists.');
    expect(nameProblem('Code', taken)).toContain('kept for Jira');
    expect(nameProblem(' ', taken)).toBe('Give it a name.');
  });
});
