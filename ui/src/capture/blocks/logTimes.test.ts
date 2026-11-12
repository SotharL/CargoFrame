import { describe, expect, it } from 'vitest';
import { splitLines } from './logLines';
import { during, hms, onDay, stamps, timeOf } from './logTimes';

const local = (h: number, mi: number, s: number, f = 0) => new Date(2026, 10, 12, h, mi, s, f).getTime();
const day = local(14, 0, 0);

describe('the time of a log line', () => {
  it('reads the ways logs write it, in the computer’s time unless the line or the block says', () => {
    expect(timeOf('2026-11-12 14:03:27 [info] up', day, false)).toBe(local(14, 3, 27));
    expect(timeOf('2026/11/12 14:03:27,123 warn', day, false)).toBe(local(14, 3, 27, 123));
    // Unreal Engine
    expect(timeOf('[2026.11.12-14.03.27:456][  0]LogInit: Display', day, false)).toBe(local(14, 3, 27, 456));
    // syslog, the year from the capture
    expect(timeOf('Nov 12 14:03:27 host app[12]: ok', day, false)).toBe(local(14, 3, 27));
    // the time alone, on the capture's day
    expect(timeOf('[14:03:27.5] tick', day, false)).toBe(local(14, 3, 27, 500));
    // a zone is read as it says
    expect(timeOf('2026-11-12T13:03:27.000Z boot', day, false)).toBe(Date.UTC(2026, 10, 12, 13, 3, 27));
    expect(timeOf('2026-11-12T14:03:27+01:00 boot', day, false)).toBe(Date.UTC(2026, 10, 12, 13, 3, 27));
    // the block says UTC
    expect(timeOf('2026-11-12 14:03:27 x', day, true)).toBe(Date.UTC(2026, 10, 12, 14, 3, 27));
  });

  it('finds no time where there is none, or one that cannot be', () => {
    expect(timeOf('    at Foo.bar (foo.js:12:7)', day, false)).toBeNull();
    expect(timeOf('version 1.2.3 build 4567', day, false)).toBeNull();
    expect(timeOf('2026-13-40 25:61:00 nope', day, false)).toBeNull();
    // a time further in than the start of the line is not the line's
    expect(timeOf(`${'x'.repeat(100)} 14:03:27`, day, false)).toBeNull();
  });

  it('gives a line without a time the time of the line before, and keeps the lines of a span', () => {
    const lines = splitLines('starting\n14:00:01 a\n  more of a\n14:00:05 b\n14:00:09 c\n');
    const t = stamps(lines, day, false);
    expect(t.map((x) => x?.at ?? null)).toEqual([null, local(14, 0, 1), local(14, 0, 1), local(14, 0, 5), local(14, 0, 9)]);
    const kept = during(lines, t, { from: local(14, 0, 1), to: local(14, 0, 6) });
    expect(kept.map((l) => l.n)).toEqual([2, 3, 4]);
  });

  it('writes and reads a time of day', () => {
    expect(hms(local(9, 5, 7))).toBe('09:05:07');
    expect(onDay(day, '14:03:27')).toBe(local(14, 3, 27));
    expect(onDay(day, '14:03')).toBe(local(14, 3, 0));
    expect(onDay(day, 'nope')).toBeNull();
  });
});
