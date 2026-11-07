import { describe, expect, it } from 'vitest';
import type { DropdownOption } from '../../lib/types';
import { ALL, NO_CATEGORY, categoriesIn, comboOptions, narrow, needsCategoryStep, recentIn, scope, serversOf, startOn } from './cascade';

const o = (value: string, label: string, server: string | null, category: string | null = null): DropdownOption => ({
  value, label, group: null, server, category,
});

const two: DropdownOption[] = [
  o('1', '#general', 'Acme'),
  o('2', '#bugs', 'Acme', 'Support'),
  o('3', '#help', 'Acme', 'Support'),
  o('4', '#ideas', 'Acme', 'Product'),
  o('5', '#lounge', 'Friends'),
  o('6', '#games', 'Friends'),
];

describe('narrowing a long list step by step', () => {
  it('lists the servers and the categories inside one', () => {
    expect(serversOf(two)).toEqual(['Acme', 'Friends']);
    expect(categoriesIn(two, 'Acme')).toEqual(['Support', 'Product']);
    expect(categoriesIn(two, 'Friends')).toEqual([]);
  });

  it('asks for a category only when there are two places to choose between', () => {
    expect(needsCategoryStep(two, 'Acme')).toBe(true);
    // Friends has no categories at all: straight to its channels.
    expect(needsCategoryStep(two, 'Friends')).toBe(false);
    // One category and nothing outside it: nothing to choose.
    expect(needsCategoryStep([o('1', '#a', 'S', 'Only'), o('2', '#b', 'S', 'Only')], 'S')).toBe(false);
    // One category plus channels outside any category are two places.
    expect(needsCategoryStep([o('1', '#a', 'S', 'Only'), o('2', '#b', 'S')], 'S')).toBe(true);
  });

  it('keeps only the choices of the server and the category picked', () => {
    expect(scope(two, 'Acme', ALL).map((x) => x.value)).toEqual(['1', '2', '3', '4']);
    expect(scope(two, 'Acme', 'Support').map((x) => x.value)).toEqual(['2', '3']);
    expect(scope(two, 'Acme', NO_CATEGORY).map((x) => x.value)).toEqual(['1']);
    expect(scope(two, 'Friends', ALL).map((x) => x.value)).toEqual(['5', '6']);
  });

  it('leaves a list without servers alone', () => {
    const flat = [o('a', 'A', null), o('b', 'B', null)];
    expect(scope(flat, null, ALL)).toHaveLength(2);
    expect(serversOf(flat)).toEqual([]);
  });

  it('starts on the last one used that still exists, else the first listed', () => {
    expect(startOn(two, [])?.value).toBe('1');
    expect(startOn(two, [{ value: '6', label: '#games' }, { value: '2', label: '#bugs' }])?.value).toBe('6');
    expect(startOn(two, [{ value: 'gone', label: '#old' }, { value: '3', label: '#help' }])?.value).toBe('3');
    expect(startOn(two, [{ value: 'gone', label: '#old' }])?.value).toBe('1');
    expect(startOn([], [])).toBeUndefined();
  });
});

describe('narrowing by the title', () => {
  const threads = [o('t1', 'Crash on save', 'Acme', '#bugs'), o('t2', 'crash in export', 'Acme', '#bugs'), o('t3', 'Ideas', 'Acme', '#chat')];

  it('keeps the threads whose name contains the words, whatever the case', () => {
    const r = narrow(threads, '  CRASH ');
    expect(r.options.map((x) => x.value)).toEqual(['t1', 't2']);
    expect(r).toMatchObject({ applied: true, matched: true });
  });

  it('shows everything, and says so, when nothing matches', () => {
    const r = narrow(threads, 'zzz');
    expect(r.options).toHaveLength(3);
    expect(r).toMatchObject({ applied: false, matched: false });
  });

  it('does nothing without text', () => {
    expect(narrow(threads, '')).toMatchObject({ applied: false, matched: true });
  });
});

describe('what the list shows', () => {
  it('puts recent choices first under their own heading, once each', () => {
    const recent = recentIn(two, [{ value: '6', label: 'x' }, { value: 'gone', label: 'y' }, { value: '2', label: 'z' }]);
    expect(recent.map((x) => x.value)).toEqual(['6', '2']);
    const list = comboOptions(scope(two, 'Acme', ALL), recent, true);
    expect(list.map((x) => `${x.group}:${x.value}`)).toEqual(['Recent:6', 'Recent:2', 'Others:1', 'Others:3', 'Others:4']);
    expect(list[0]!.note).toBe('Friends');
    expect(list[1]!.note).toBe('Acme · Support');
  });

  it('is a plain list when nothing was used yet', () => {
    const list = comboOptions(two, [], false);
    expect(list.every((x) => x.group === null && x.note === null)).toBe(true);
  });
});
