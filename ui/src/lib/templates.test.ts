// @vitest-environment node
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { memoryApi } from './memoryApi';

// The ready-made layouts the in-memory stand-in lists (demos, tests, screenshots) are the
// program's own: same keys, names and sentences, in the same order.
const rs = readFileSync(new URL('../../../core/layout/src/templates.rs', import.meta.url), 'utf8');
const program = [...rs.matchAll(/key: "([^"]+)",\s*name: msg!\("([^"]+)"\),\s*description: msg!\(\s*"((?:[^"\\]|\\.)*)",?\s*\)/g)].map((m) => ({
  key: m[1],
  name: m[2],
  description: m[3]!.replace(/\\"/g, '"'),
}));

describe('the ready-made layouts of the stand-in', () => {
  it('are the program’s', async () => {
    const listed = await memoryApi({ general: { debug: true } }).listLayoutTemplates();
    expect(program.length).toBe(21);
    expect(listed.map(({ key, name, description }) => ({ key, name, description }))).toEqual(program);
  });

  it('offer the CargoFrame bug report in debug mode only', async () => {
    const keys = (await memoryApi().listLayoutTemplates()).map((t) => t.key);
    expect(keys).not.toContain('cargoframe-report');
    expect(keys.length).toBe(20);
  });
});
