import { describe, expect, it } from 'vitest';
import { compile, example, isFormat, readWith, type TimeFormat } from './logFormat';

const local = (y: number, mo: number, d: number, h: number, mi: number, s = 0, ms = 0) => new Date(y, mo - 1, d, h, mi, s, ms).getTime();
const day = local(2026, 11, 12, 9, 0);
const fmt = (s: string): TimeFormat => {
  const f = compile(s);
  if (!isFormat(f)) throw new Error(f.problem);
  return f;
};

describe('a time format the person writes', () => {
  it('reads the formats people write, filling in what they leave out from the capture', () => {
    expect(readWith(fmt('[YYYY-MM-DD HH:mm]'), '[2026-11-12 14:03] boot', day, false)).toBe(local(2026, 11, 12, 14, 3));
    // no year, no day: those of the capture
    expect(readWith(fmt('MM_DD_HH_mm_ss'), '11_12_14_03_27 tick', day, false)).toBe(local(2026, 11, 12, 14, 3, 27));
    expect(readWith(fmt('[YYYY_MM_DD] HH:mm -'), '[2026_11_12] 14:03 - Game started', day, false)).toBe(local(2026, 11, 12, 14, 3));
    expect(readWith(fmt('HH:mm:ss'), 'INFO 14:03:27 ok', day, false)).toBe(local(2026, 11, 12, 14, 3, 27));
    // a counter before the time, and milliseconds
    expect(readWith(fmt('[#][YYYY.MM.DD-HH.mm.ss:SSS]'), '[  12][2026.11.12-14.03.27:045]LogInit', day, false)).toBe(
      local(2026, 11, 12, 14, 3, 27, 45),
    );
    // month names, a 12-hour clock, a zone, seconds since 1970
    expect(readWith(fmt('DD MMM YYYY hh:mm:ss A'), '12 Nov 2026 02:03:27 PM x', day, false)).toBe(local(2026, 11, 12, 14, 3, 27));
    expect(readWith(fmt('YYYY-MM-DDTHH:mm:ssZ'), '2026-11-12T13:03:27+01:00', day, false)).toBe(Date.UTC(2026, 10, 12, 12, 3, 27));
    expect(readWith(fmt('X'), '1794480207.5 ping', day, false)).toBe(1_794_480_207_500);
    // spaces stretch; lowercase words are themselves, and letters in quotes too
    expect(readWith(fmt('at HH:mm'), 'done at    14:03', day, false)).toBe(local(2026, 11, 12, 14, 3));
    expect(readWith(fmt("'Day' DD, HH:mm"), 'Day 12, 14:03', day, false)).toBe(local(2026, 11, 12, 14, 3));
    // UTC when said
    expect(readWith(fmt('HH:mm'), '14:03', day, true)).toBe(Date.UTC(2026, 10, 12, 14, 3));
  });

  it('knows how long a written time lasts, and finds nothing where the format is not', () => {
    expect(fmt('HH:mm').unit).toBe(60_000);
    expect(fmt('HH:mm:ss').unit).toBe(1_000);
    expect(fmt('HH:mm:ss.SSS').unit).toBe(1);
    expect(fmt('YYYY-MM-DD').unit).toBe(86_400_000);
    expect(readWith(fmt('[YYYY-MM-DD HH:mm]'), '2026-11-12 14:03', day, false)).toBeNull();
    expect(readWith(fmt('HH:mm'), '99:99', day, false)).toBeNull();
  });

  it('says what is wrong with a format', () => {
    for (const [bad, why] of [
      ['', /Write a format/],
      ['[foo]', /no time/],
      ["'at HH:mm", /quote/],
    ] as const) {
      const f = compile(bad);
      expect(isFormat(f) ? '' : f.problem).toMatch(why);
    }
  });

  it('shows what a time looks like in the format', () => {
    const at = local(2026, 11, 12, 14, 3, 27, 45);
    expect(example(fmt('[YYYY_MM_DD] HH:mm -'), at)).toBe('[2026_11_12] 14:03 -');
    expect(example(fmt('MM_DD_HH_mm_ss'), at)).toBe('11_12_14_03_27');
    expect(example(fmt('[#][YYYY.MM.DD-HH.mm.ss:SSS]'), at)).toBe('[12][2026.11.12-14.03.27:045]');
    expect(example(fmt('DD MMM hh:mm A'), at)).toBe('12 Nov 02:03 PM');
  });
});
