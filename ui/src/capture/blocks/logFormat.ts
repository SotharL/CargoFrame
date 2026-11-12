// A time format the person writes for a log, the way the log writes its times:
//
//   [YYYY-MM-DD HH:mm]      MM_DD_HH_mm_ss      [YYYY_MM_DD] HH:mm -      [#][YYYY.MM.DD-HH.mm.ss:SSS]
//
// The letters stand for the parts of a date (below); everything else is matched as written, and
// a space matches any run of spaces. Only the time needs writing: it is looked for near the start
// of each line. What the format leaves out is filled in from the capture: the day, month or year,
// and a line with no seconds covers its whole minute.
//
//   YYYY / YY  year          MM / M  month        MMM  month name (Jan)
//   DD / D     day           HH / H  hour         hh  hour with A (AM/PM)
//   mm         minutes       ss      seconds      SSS (as many S as written)  part of a second
//   Z          zone (Z, +01:00)                   X  seconds since 1970
//   #          any number (a counter, a frame)    *  anything
//
// Other lowercase letters are themselves; letters meant as themselves that are tokens go in
// quotes ('Date') or after a backslash.

export type FormatPart =
  | 'year4' | 'year2' | 'month' | 'monthName' | 'day' | 'hour' | 'hour12' | 'ampm'
  | 'minute' | 'second' | 'fraction' | 'zone' | 'unix';

/** A format ready to read lines with. */
export interface TimeFormat {
  source: string;
  regex: RegExp;
  parts: FormatPart[];
  /** How long one written time lasts: a line with minutes only covers its minute. */
  unit: number;
}

const MONTHS = ['jan', 'feb', 'mar', 'apr', 'may', 'jun', 'jul', 'aug', 'sep', 'oct', 'nov', 'dec'];

/** The tokens, longest first so `YYYY` is not read as two `YY`. */
const TOKENS: [string, FormatPart | '#' | '*', string][] = [
  ['YYYY', 'year4', '(\\d{4})'],
  ['yyyy', 'year4', '(\\d{4})'],
  ['MMMM', 'monthName', '([A-Za-z]{3,9})'],
  ['MMM', 'monthName', '([A-Za-z]{3,9})'],
  ['YY', 'year2', '(\\d{2})'],
  ['yy', 'year2', '(\\d{2})'],
  ['MM', 'month', '(\\d{2})'],
  ['DD', 'day', '(\\d{2})'],
  ['dd', 'day', '(\\d{2})'],
  ['HH', 'hour', '(\\d{2})'],
  ['hh', 'hour12', '(\\d{2})'],
  ['mm', 'minute', '(\\d{2})'],
  ['ss', 'second', '(\\d{2})'],
  ['M', 'month', '(\\d{1,2})'],
  ['D', 'day', '(\\d{1,2})'],
  ['H', 'hour', '(\\d{1,2})'],
  ['A', 'ampm', '([AaPp][Mm])'],
  ['Z', 'zone', '(Z|[+-]\\d{2}:?\\d{2})'],
  ['X', 'unix', '(\\d{9,11}(?:\\.\\d+)?)'],
  ['#', '#', '\\s*\\d+'],
  ['*', '*', '.*?'],
];

const escape = (c: string) => c.replace(/[.*+?^${}()|[\]\\/]/g, '\\$&');

/** Read a format, or say what is wrong with it. */
export function compile(source: string): TimeFormat | { problem: string } {
  const text = source.trim();
  if (!text) return { problem: 'Write a format, such as [YYYY-MM-DD HH:mm:ss]' };
  let re = '';
  const parts: FormatPart[] = [];
  let i = 0;
  while (i < text.length) {
    const c = text[i]!;
    if (c === '\\' && i + 1 < text.length) {
      re += escape(text[i + 1]!);
      i += 2;
      continue;
    }
    if (c === "'") {
      const end = text.indexOf("'", i + 1);
      if (end < 0) return { problem: 'A quote is not closed' };
      re += escape(text.slice(i + 1, end));
      i = end + 1;
      continue;
    }
    if (/\s/.test(c)) {
      while (i < text.length && /\s/.test(text[i]!)) i++;
      re += '\\s+';
      continue;
    }
    // a part of a second: as many S as written
    if (c === 'S') {
      let n = 0;
      while (text[i + n] === c) n++;
      re += `(\\d{1,${Math.max(n, 9)}})`;
      parts.push('fraction');
      i += n;
      continue;
    }
    const token = TOKENS.find(([t]) => text.startsWith(t, i));
    if (token) {
      const [t, part, pattern] = token;
      re += pattern;
      if (part !== '#' && part !== '*') parts.push(part);
      i += t.length;
      continue;
    }
    re += escape(c);
    i++;
  }
  const has = (p: FormatPart) => parts.includes(p);
  const timeParts: FormatPart[] = ['hour', 'hour12', 'minute', 'second', 'unix', 'day', 'monthName', 'month'];
  if (!parts.some((p) => timeParts.includes(p))) {
    return { problem: 'The format has no time in it: use HH for the hour, mm for the minutes…' };
  }
  if (has('hour12') && !has('ampm')) {
    // a 12-hour clock needs AM or PM; without it, hh is read as a 24-hour clock
    parts[parts.indexOf('hour12')] = 'hour';
  }
  const unit = has('fraction')
    ? 1
    : has('second') || has('unix')
      ? 1_000
      : has('minute')
        ? 60_000
        : has('hour') || has('hour12')
          ? 3_600_000
          : 86_400_000;
  try {
    return { source: text, regex: new RegExp(re), parts, unit };
  } catch {
    return { problem: 'This format cannot be read' };
  }
}

export const isFormat = (f: TimeFormat | { problem: string }): f is TimeFormat => 'regex' in f;

/** Where a time is looked for: near the start of a line. */
const HEAD = 160;

/**
 * The time a line was written, by the format, in ms since 1970; what the format leaves out comes
 * from `day` (the capture). A time without a zone is this computer's, or UTC.
 */
export function readWith(format: TimeFormat, line: string, day: number, utc: boolean): number | null {
  const m = format.regex.exec(line.slice(0, HEAD));
  if (!m) return null;
  const ref = new Date(day);
  let year = utc ? ref.getUTCFullYear() : ref.getFullYear();
  let month = (utc ? ref.getUTCMonth() : ref.getMonth()) + 1;
  let date = utc ? ref.getUTCDate() : ref.getDate();
  let hour = 0;
  let minute = 0;
  let second = 0;
  let ms = 0;
  let pm: boolean | null = null;
  let zone: string | null = null;
  for (let i = 0; i < format.parts.length; i++) {
    const v = m[i + 1] ?? '';
    switch (format.parts[i]) {
      case 'year4': year = +v; break;
      case 'year2': year = 2000 + +v; break;
      case 'month': month = +v; break;
      case 'monthName': month = MONTHS.indexOf(v.slice(0, 3).toLowerCase()) + 1; break;
      case 'day': date = +v; break;
      case 'hour': case 'hour12': hour = +v; break;
      case 'ampm': pm = v.toLowerCase() === 'pm'; break;
      case 'minute': minute = +v; break;
      case 'second': second = +v; break;
      case 'fraction': ms = Number((v + '00').slice(0, 3)); break;
      case 'zone': zone = v; break;
      case 'unix': return Math.round(Number(v) * 1000);
    }
  }
  if (pm !== null) hour = (hour % 12) + (pm ? 12 : 0);
  if (month < 1 || month > 12 || date < 1 || date > 31 || hour > 23 || minute > 59 || second > 60) return null;
  if (zone) {
    const base = Date.UTC(year, month - 1, date, hour, minute, second, ms);
    if (zone === 'Z') return base;
    const z = zone.replace(':', '');
    const offset = (Number(z.slice(1, 3)) * 60 + Number(z.slice(3, 5))) * 60_000;
    return z[0] === '+' ? base - offset : base + offset;
  }
  return utc ? Date.UTC(year, month - 1, date, hour, minute, second, ms) : new Date(year, month - 1, date, hour, minute, second, ms).getTime();
}

/** What a time looks like in the format, to show the person as they write it. */
export function example(format: TimeFormat, at: number, utc = false): string {
  const d = new Date(at);
  const get = {
    year: utc ? d.getUTCFullYear() : d.getFullYear(),
    month: (utc ? d.getUTCMonth() : d.getMonth()) + 1,
    day: utc ? d.getUTCDate() : d.getDate(),
    hour: utc ? d.getUTCHours() : d.getHours(),
    minute: utc ? d.getUTCMinutes() : d.getMinutes(),
    second: utc ? d.getUTCSeconds() : d.getSeconds(),
    ms: utc ? d.getUTCMilliseconds() : d.getMilliseconds(),
  };
  const pad = (n: number, w: number) => String(n).padStart(w, '0');
  const text = format.source;
  let out = '';
  let i = 0;
  const ampm = format.parts.includes('ampm');
  while (i < text.length) {
    const c = text[i]!;
    if (c === '\\' && i + 1 < text.length) {
      out += text[i + 1];
      i += 2;
      continue;
    }
    if (c === "'") {
      const end = text.indexOf("'", i + 1);
      out += text.slice(i + 1, end < 0 ? undefined : end);
      i = end < 0 ? text.length : end + 1;
      continue;
    }
    if (c === 'S') {
      let n = 0;
      while (text[i + n] === c) n++;
      out += pad(get.ms, 3).padEnd(n, '0').slice(0, n);
      i += n;
      continue;
    }
    const token = TOKENS.find(([t]) => text.startsWith(t, i));
    if (!token) {
      out += c;
      i++;
      continue;
    }
    const [t] = token;
    const h12 = get.hour % 12 || 12;
    const value: Record<string, string> = {
      YYYY: String(get.year), yyyy: String(get.year), YY: pad(get.year % 100, 2), yy: pad(get.year % 100, 2),
      MMMM: d.toLocaleString('en', { month: 'long', timeZone: utc ? 'UTC' : undefined }),
      MMM: d.toLocaleString('en', { month: 'short', timeZone: utc ? 'UTC' : undefined }),
      MM: pad(get.month, 2), M: String(get.month), DD: pad(get.day, 2), dd: pad(get.day, 2), D: String(get.day),
      HH: pad(get.hour, 2), H: String(get.hour), hh: pad(ampm ? h12 : get.hour, 2),
      mm: pad(get.minute, 2), ss: pad(get.second, 2),
      A: get.hour < 12 ? 'AM' : 'PM', Z: utc ? 'Z' : zoneOf(d),
      X: String(Math.floor(at / 1000)), '#': '12', '*': '…',
    };
    out += value[t] ?? t;
    i += t.length;
  }
  return out;
}

function zoneOf(d: Date): string {
  const off = -d.getTimezoneOffset();
  const sign = off >= 0 ? '+' : '-';
  const a = Math.abs(off);
  return `${sign}${String(Math.floor(a / 60)).padStart(2, '0')}:${String(a % 60).padStart(2, '0')}`;
}

/** Formats seen often, offered while writing one. */
export const COMMON_FORMATS = [
  '[YYYY.MM.DD-HH.mm.ss:SSS]',
  'YYYY-MM-DD HH:mm:ss',
  '[YYYY-MM-DD HH:mm:ss]',
  'YYYY-MM-DD HH:mm:ss,SSS',
  'YYYY-MM-DDTHH:mm:ss.SSSZ',
  'MMM D HH:mm:ss',
  '[HH:mm:ss]',
];

/** The letters a format can use, said briefly (shown next to a format box). */
export const FORMAT_HELP =
  "YYYY year, MM month, DD day, HH hour, mm minutes, ss seconds, SSS milliseconds, A AM/PM, Z zone, # a number, * anything; the rest is matched as written ('quotes' for letters meant as themselves)";
