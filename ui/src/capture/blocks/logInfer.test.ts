import { describe, expect, it } from 'vitest';
import { compile, isFormat, readWith } from './logFormat';
import { inferFormat } from './logInfer';

const local = (y: number, mo: number, d: number, h: number, mi: number, s = 0, ms = 0) => new Date(y, mo - 1, d, h, mi, s, ms).getTime();
const day = local(2026, 11, 12, 9, 0);
const read = (format: string, line: string) => {
  const f = compile(format);
  return isFormat(f) ? readWith(f, line, day, false) : null;
};

describe('a time format from a selected time', () => {
  it('writes the format of the times people select', () => {
    expect(inferFormat('[2026.11.12-14.03.27:045]', day)).toBe('[YYYY.MM.DD-HH.mm.ss:SSS]');
    expect(inferFormat('2026-11-12 14:03:27', day)).toBe('YYYY-MM-DD HH:mm:ss');
    expect(inferFormat('2026-11-12 14:03:27,123', day)).toBe('YYYY-MM-DD HH:mm:ss,SSS');
    expect(inferFormat('14:03:27', day)).toBe('HH:mm:ss');
    expect(inferFormat('[14:03]', day)).toBe('[HH:mm]');
    expect(inferFormat('Nov 12 14:03:27', day)).toBe('MMM DD HH:mm:ss');
    expect(inferFormat('11_12_14_03_27', day)).toBe('MM_DD_HH_mm_ss');
    expect(inferFormat('1794480207', day)).toBe('X');
  });

  it('reads a day and a month in the order they can only be in, or as the computer writes them', () => {
    expect(inferFormat('25/11/2026 14:03', day, false, true)).toBe('DD/MM/YYYY HH:mm');
    expect(inferFormat('11/25/2026 14:03', day, false, false)).toBe('MM/DD/YYYY HH:mm');
    expect(inferFormat('11/12/2026 2:03:27 PM', day, false, true)).toBe('MM/DD/YYYY H:mm:ss A');
    expect(inferFormat('11/12/2026 2:03:27 PM', day, false, false)).toBe('DD/MM/YYYY H:mm:ss A');
  });

  it('keeps a zone and writes letters that mean something as themselves', () => {
    const iso = inferFormat('2026-11-12T13:03:27+01:00', day)!;
    expect(iso).toBe('YYYY-MM-DDTHH:mm:ssZ');
    expect(read(iso, '2026-11-12T13:03:27+01:00 boot')).toBe(Date.UTC(2026, 10, 12, 12, 3, 27));
    expect(inferFormat('2026-11-12T13:03:27Z', day)).toBe('YYYY-MM-DDTHH:mm:ssZ');
    expect(inferFormat('Day 12, 14:03', day)).toBe('\\Da\\y DD, HH:mm');
  });

  it('the format found reads the line it came from', () => {
    const line = '[  12][2026.11.12-14.03.27:045]LogInit: Display: Running engine';
    const f = inferFormat('2026.11.12-14.03.27:045', day)!;
    expect(read(f, line)).toBe(local(2026, 11, 12, 14, 3, 27, 45));
  });

  it('gives nothing for what is not a time', () => {
    for (const bad of ['', 'LogInit', 'Error 404', '1.2.3.4.5.6.7', 'a'.repeat(80)]) expect(inferFormat(bad, day)).toBeNull();
  });
});
