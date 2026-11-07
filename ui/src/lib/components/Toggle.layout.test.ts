// @vitest-environment node
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

// The hidden input of a Toggle is absolutely positioned. With no positioned parent it escapes the
// scrolling page and gives the whole window a second scroll bar (it did, on the General page).
// jsdom has no layout, so this checks the rule that prevents it.
describe('Toggle layout', () => {
  it('is the positioned parent of its hidden input', () => {
    const source = readFileSync(new URL('./Toggle.svelte', import.meta.url), 'utf8');
    const rule = source.match(/\.toggle\s*\{([^}]*)\}/);
    expect(rule?.[1]).toMatch(/position:\s*relative/);
  });
});
