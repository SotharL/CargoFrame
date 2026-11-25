// @vitest-environment node
//
// Lists and limits the window keeps a copy of, read from the Rust source and compared with the
// copy: a value group added on one side only (0.7.2: `system.` and `cargoframe.`, which left
// debug mode's values unread) fails here instead of on the owner's PC. One test per pair; a new
// copy of a Rust list gets its line here.
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { GROUPS, RESERVED } from './valueNames';
import { MAX_TAGS, MAX_TAG_CHARS } from './types';
import { MIN_TABS, MAX_TABS, MIN_TABS_SIZE } from './tabs';
import { MAX_SHARING_BUTTONS, BLOCK_TYPES } from '../settings/layout/model';
import { groupOf } from '../settings/layout/automation';
import { memoryApi } from './ipc';

const rust = (path: string) => readFileSync(new URL(`../../../${path}`, import.meta.url), 'utf8');

/** The strings of a Rust array constant: `const NAME: [&str; N] = ["a", "b"];`. */
function strings(src: string, name: string): string[] {
  const m = new RegExp(`const ${name}: \\[&str; \\d+\\] = \\[([\\s\\S]*?)\\];`).exec(src);
  if (!m) throw new Error(`${name} not found`);
  const out = [...m[1]!.replace(/\/\/[^\n]*/g, '').matchAll(/"([^"]*)"/g)].map((x) => x[1]!);
  if (!out.length) throw new Error(`${name} read as empty`);
  return out;
}

/** A Rust number constant: `pub const NAME: usize = 25;`. */
function number(src: string, name: string): number {
  const m = new RegExp(`const ${name}: \\w+ = (\\d+);`).exec(src);
  if (!m) throw new Error(`${name} not found`);
  return Number(m[1]);
}

describe('the window’s copies of the program’s lists', () => {
  const template = rust('core/context/src/template.rs');
  const model = rust('core/layout/src/model.rs');
  const vars = rust('core/window/src/vars.rs');
  const debug = rust('app/src/debug.rs');

  it('read the same value groups and leave Jira’s own names alone (template.rs)', () => {
    expect(GROUPS).toEqual(strings(template, 'GROUPS'));
    expect(RESERVED).toEqual(strings(template, 'RESERVED'));
  });

  it('have the same limits (core/layout)', () => {
    expect(MAX_TAGS).toBe(number(model, 'MAX_TAGS'));
    expect(MAX_TAG_CHARS).toBe(number(model, 'MAX_TAG_CHARS'));
    expect(MIN_TABS).toBe(number(model, 'MIN_TABS'));
    expect(MAX_TABS).toBe(number(model, 'MAX_TABS'));
    expect(MIN_TABS_SIZE).toBe(number(model, 'MIN_TABS_SIZE'));
    expect(MAX_SHARING_BUTTONS).toBe(number(rust('core/layout/src/lib.rs'), 'MAX_SHARING_BUTTONS'));
  });

  it('offer every kind of block (BlockKind)', () => {
    const kinds = /pub enum BlockKind \{([\s\S]*?)\n\}/.exec(model)![1]!
      .split('\n')
      .map((l) => /^\s{4}([A-Z]\w*)/.exec(l)?.[1])
      .filter((k): k is string => !!k)
      .map((k) => k.replace(/([a-z])([A-Z])/g, '$1_$2').toLowerCase());
    expect(BLOCK_TYPES.map((b) => b.type).sort()).toEqual(kinds.sort());
  });

  it('know every value CargoFrame fills, each in its group (vars.rs, debug.rs)', async () => {
    const names = [...strings(vars, 'NAMES'), ...strings(debug, 'NAMES')];
    for (const n of names) {
      expect(GROUPS.some((g) => n.startsWith(g)), n).toBe(true);
      expect(groupOf(n), n).not.toBe('Other');
    }
    // the demo lists the program's values, so screenshots and tests show what a capture has
    const demo = new Set((await memoryApi().listVariables()).map((v) => v.name));
    for (const n of strings(vars, 'NAMES')) expect(demo.has(n), n).toBe(true);
  });
});
