// The guides point at things in the pages' code by name. When a field is renamed or removed,
// the guide that points at it must change too: these tests fail until it does.
// @vitest-environment node
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import connections from '../pages/Connections.svelte?raw';
import { isPageId } from '../nav';
import { MAX_SAY, TOURS, tourById } from './tours';

// Every page's code, as text: the guides name things by their `data-tour` attribute.
const pages = import.meta.glob('../../**/*.svelte', { query: '?raw', import: 'default', eager: true }) as Record<string, string>;
const code = Object.values(pages).join('\n');
const named = (target: string) => code.includes(`data-tour="${target}"`) || code.includes(`tour="${target}"`);

/** The keys of the ready-made layouts, as the program has them. */
const templatesRs = readFileSync(new URL('../../../../core/layout/src/templates.rs', import.meta.url), 'utf8');
const templateKeys = [...templatesRs.matchAll(/key: "([a-z-]+)"/g)].map((m) => m[1]!);
/** The services a connection can be made for (the buttons of "Add a connection"). */
const list = connections.slice(connections.indexOf('const KINDS'), connections.indexOf('];', connections.indexOf('const KINDS')));
const kinds = [...list.matchAll(/kind: '([a-z]+)', label:/g)].map((m) => m[1]!);

describe('the guides', () => {
  it('have their own ids and each starts on a page, so it can start from anywhere', () => {
    expect(new Set(TOURS.map((t) => t.id)).size).toBe(TOURS.length);
    for (const t of TOURS) {
      expect(t.steps.length, t.id).toBeGreaterThan(1);
      expect(t.steps[0]!.page, `${t.id} starts on a page`).toBeTruthy();
      for (const s of t.steps) if (s.page) expect(isPageId(s.page), `${t.id}: ${s.page}`).toBe(true);
    }
  });

  it('say one short thing per step', () => {
    for (const t of TOURS) {
      for (const s of t.steps) {
        expect(s.say.length, `${t.id}: ${s.say}`).toBeLessThanOrEqual(MAX_SAY);
        expect(s.say.trim(), t.id).not.toBe('');
      }
    }
  });

  it('only point at things that are in the pages', () => {
    const lost: string[] = [];
    for (const t of TOURS) {
      for (const s of t.steps) {
        const add = /^add-(.+)$/.exec(s.target);
        const template = /^template-(.+)$/.exec(s.target);
        const fine = add
          ? kinds.includes(add[1]!) && code.includes('data-tour={`add-${')
          : template
            ? templateKeys.includes(template[1]!) && code.includes('data-tour={`template-${')
            : named(s.target);
        if (!fine) lost.push(`${t.id}: ${s.target}`);
      }
    }
    expect(lost).toEqual([]);
  });

  it('cover every service a connection can be made for', () => {
    expect(kinds.length).toBeGreaterThan(5);
    for (const k of kinds) expect(tourById(`connect-${k}`), k).toBeDefined();
  });
});
