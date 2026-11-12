// The time each line of a log was written, as the line itself says, so a log viewer can keep the
// lines of a moment (the recording, or around the capture). Read from the start of the line:
//
//   2026-11-12 14:03:27        2026-11-12T14:03:27.123Z      2026/11/12 14:03:27,123
//   [2026.11.12-14.03.27:123]  (Unreal Engine)               Nov 12 14:03:27 (syslog)
//   [14:03:27] or 14:03:27.5   (the time alone: the day is the capture's)
//
// A time with its zone (Z, +01:00) is read as it says; one without is the computer's own time,
// or UTC when the block says the files are written in UTC. A line with no time (the rest of a
// message, a stack trace) has the time of the line before it.
import type { LogLine } from './logLines';

const MONTHS = ['jan', 'feb', 'mar', 'apr', 'may', 'jun', 'jul', 'aug', 'sep', 'oct', 'nov', 'dec'];

/** Where a time is looked for: the start of a line. */
const HEAD = 80;

const FULL = /(\d{4})[-/.](\d{2})[-/.](\d{2})[T _-](\d{2})[:.](\d{2})[:.](\d{2})(?:[.,:](\d{1,9}))?\s*(Z|[+-]\d{2}:?\d{2})?/;
const SYSLOG = /\b(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s+(\d{1,2})\s+(\d{2}):(\d{2}):(\d{2})\b/i;
const CLOCK = /(?:^|[^\d:])(\d{2}):(\d{2}):(\d{2})(?:[.,](\d{1,6}))?(?![\d:])/;

const ms = (frac: string | undefined) => (frac ? Number((frac + '00').slice(0, 3)) : 0);

/** Milliseconds since 1970 of a date and time, in UTC or in this computer's zone. */
function at(y: number, mo: number, d: number, h: number, mi: number, s: number, f: number, utc: boolean): number | null {
  if (mo < 1 || mo > 12 || d < 1 || d > 31 || h > 23 || mi > 59 || s > 60) return null;
  return utc ? Date.UTC(y, mo - 1, d, h, mi, s, f) : new Date(y, mo - 1, d, h, mi, s, f).getTime();
}

/**
 * The time written at the start of a line, in ms since 1970, or `null` when it has none.
 * `day` is the capture's moment: it gives the year to a syslog time and the day to a time alone.
 */
export function timeOf(line: string, day: number, utc: boolean): number | null {
  const head = line.slice(0, HEAD);
  const full = FULL.exec(head);
  if (full) {
    const [, y, mo, d, h, mi, s, f, zone] = full;
    if (zone) {
      const base = at(+y!, +mo!, +d!, +h!, +mi!, +s!, ms(f), true);
      if (base === null) return null;
      if (zone === 'Z') return base;
      const z = zone.replace(':', '');
      const offset = (Number(z.slice(1, 3)) * 60 + Number(z.slice(3, 5))) * 60_000;
      return z[0] === '+' ? base - offset : base + offset;
    }
    return at(+y!, +mo!, +d!, +h!, +mi!, +s!, ms(f), utc);
  }
  const ref = new Date(day);
  const year = utc ? ref.getUTCFullYear() : ref.getFullYear();
  const sys = SYSLOG.exec(head);
  if (sys) {
    const [, mon, d, h, mi, s] = sys;
    return at(year, MONTHS.indexOf(mon!.toLowerCase()) + 1, +d!, +h!, +mi!, +s!, 0, utc);
  }
  const clock = CLOCK.exec(head);
  if (clock) {
    const [, h, mi, s, f] = clock;
    const mo = (utc ? ref.getUTCMonth() : ref.getMonth()) + 1;
    const d = utc ? ref.getUTCDate() : ref.getDate();
    return at(year, mo, d, +h!, +mi!, +s!, ms(f), utc);
  }
  return null;
}

/** Each line's time; a line without one has the time of the line before it (`null` before the first). */
export function stamps(lines: readonly LogLine[], day: number, utc: boolean): (number | null)[] {
  let last: number | null = null;
  return lines.map((l) => {
    const t = timeOf(l.text, day, utc);
    if (t !== null) last = t;
    return last;
  });
}

/** A part of the day, in ms since 1970. */
export interface Span {
  from: number;
  to: number;
}

/** The lines written during `span`, given their times. */
export function during(lines: readonly LogLine[], times: readonly (number | null)[], span: Span): LogLine[] {
  return lines.filter((_, i) => {
    const t = times[i];
    return t !== null && t !== undefined && t >= span.from && t <= span.to;
  });
}

/** A moment as a time of day on this computer (14:03:27). */
export function hms(ms: number): string {
  const d = new Date(ms);
  const two = (n: number) => String(n).padStart(2, '0');
  return `${two(d.getHours())}:${two(d.getMinutes())}:${two(d.getSeconds())}`;
}

/** A time of day typed in a time box (14:03 or 14:03:27), on the day of `day`, in ms since 1970. */
export function onDay(day: number, time: string): number | null {
  const m = /^(\d{2}):(\d{2})(?::(\d{2}))?$/.exec(time);
  if (!m) return null;
  const d = new Date(day);
  d.setHours(+m[1]!, +m[2]!, +(m[3] ?? 0), 0);
  return d.getTime();
}
