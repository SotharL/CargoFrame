// @vitest-environment node
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { memoryApi } from './ipc';

// The ready-made layouts the in-memory stand-in lists (demos, tests, screenshots) are the
// program's own: same keys, names and sentences, in the same order.
const rs = readFileSync(new URL('../../../core/layout/src/templates.rs', import.meta.url), 'utf8');
const program = [...rs.matchAll(/key: "([^"]+)",\s*name: "([^"]+)",\s*description: "((?:[^"\\]|\\.)*)"/g)].map((m) => ({
  key: m[1],
  name: m[2],
  description: m[3]!.replace(/\\"/g, '"'),
}));

describe('the ready-made layouts of the stand-in', () => {
  it('are the program’s', async () => {
    const listed = await memoryApi().listLayoutTemplates();
    expect(program.length).toBe(20);
    expect(listed.map(({ key, name, description }) => ({ key, name, description }))).toEqual(program);
  });
});
