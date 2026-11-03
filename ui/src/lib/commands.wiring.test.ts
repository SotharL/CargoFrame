// @vitest-environment node
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const rust = readFileSync(new URL('../../../app/src/commands.rs', import.meta.url), 'utf8');
const ipc = readFileSync(new URL('./ipc.ts', import.meta.url), 'utf8');

// The page calls the program by name. A name written wrongly, or a command nobody calls any more,
// shows up only when someone uses it; this keeps the two sides in step (the Rust side has its own
// test that every command is registered, listed and given to a window).

const defined = new Set(
  rust
    .split('#[tauri::command]')
    .slice(1)
    .map((after: string) => /fn\s+(\w+)/.exec(after)?.[1])
    .filter((n): n is string => !!n),
);
const called = new Set([...ipc.matchAll(/invoke(?:<[^>]*>)?\(\s*'(\w+)'/g)].map((m) => m[1]!));

describe('commands the page calls', () => {
  it('finds the commands on both sides', () => {
    expect(defined.size).toBeGreaterThan(50);
    expect(called.size).toBeGreaterThan(50);
  });

  it('are all commands the program has', () => {
    expect([...called].filter((n) => !defined.has(n))).toEqual([]);
  });

  it('cover every command the program has', () => {
    expect([...defined].filter((n) => !called.has(n))).toEqual([]);
  });
});
