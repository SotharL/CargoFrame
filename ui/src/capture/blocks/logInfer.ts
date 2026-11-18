// The time format of a log, from a time the person selected in one of its lines:
//
//   2026.11.18-14.32.05:123   ->  YYYY.MM.DD-HH.mm.ss:SSS
//   [11/18/2026 2:32:05 PM]   ->  [MM/DD/YYYY H:mm:ss A]
//   Nov 18 14:32:05           ->  MMM DD HH:mm:ss
//   14:32:05,123              ->  HH:mm:ss,SSS
//
// The numbers are read in the order people write them: a four-digit year first means year,
// month, day; a year last means day, month, year (month, day, year where this computer's
// language writes it so, or when the first number cannot be a month). What comes after the date is
// the time: hours, minutes, seconds, then a part of a second. Anything else is kept as written.
// A guess is only given when it reads the selection back.
import { compile, isFormat, readWith, type TimeFormat } from './logFormat';

const MONTHS = ['jan', 'feb', 'mar', 'apr', 'may', 'jun', 'jul', 'aug', 'sep', 'oct', 'nov', 'dec'];
/** The characters that mean something in a format; written as themselves after a backslash. */
const SPECIAL = /[YyMDdHhmsSAZX#*'\\]/;

type Piece = { kind: 'num' | 'word' | 'sep'; text: string };

function pieces(text: string): Piece[] {
  return [...text.matchAll(/\d+|[A-Za-z]+|[^\dA-Za-z]+/g)].map(([t]) => ({
    kind: /^\d/.test(t) ? 'num' : /^[A-Za-z]/.test(t) ? 'word' : 'sep',
    text: t,
  }));
}

const literal = (text: string) => [...text].map((c) => (SPECIAL.test(c) ? `\\${c}` : c)).join('');
const two = (n: Piece) => n.text.length === 2;

/** Does this computer's language write the month before the day (11/18/2026)? */
function monthFirst(): boolean {
  try {
    const parts = new Intl.DateTimeFormat(undefined, { year: 'numeric', month: '2-digit', day: '2-digit' }).formatToParts(new Date(2026, 10, 18));
    return parts.findIndex((p) => p.type === 'month') < parts.findIndex((p) => p.type === 'day');
  } catch {
    return false;
  }
}

/** The format of a selected time, or `null` when it does not look like one. */
export function inferFormat(selected: string, day: number, utc = false, mdy = monthFirst()): string | null {
  const text = selected.trim();
  if (!text || text.length > 60) return null;
  const all = pieces(text);
  const nums = all.filter((p) => p.kind === 'num');
  if (nums.length === 0) return null;
  const tokens = new Map<Piece, string>();

  // seconds since 1970, alone
  if (nums.length <= 2 && /^\d{9,11}$/.test(nums[0]!.text)) {
    tokens.set(nums[0]!, 'X');
    if (nums[1]) tokens.set(nums[1], '');
    return check(all, tokens, text, day, utc);
  }

  const words = all.filter((p) => p.kind === 'word');
  const monthName = words.find((w) => MONTHS.includes(w.text.slice(0, 3).toLowerCase()) && w.text.length <= 9);
  const ampm = words.find((w) => /^[ap]m$/i.test(w.text));
  if (monthName) tokens.set(monthName, monthName.text.length > 3 ? 'MMMM' : 'MMM');
  if (ampm) tokens.set(ampm, 'A');
  // a zone at the end: Z, or +01:00 / -0500 after the time
  const last = all[all.length - 1]!;
  if (last.kind === 'word' && last.text === 'Z') tokens.set(last, 'Z');

  let rest = [...nums];
  const year = rest.findIndex((n) => n.text.length === 4);
  const dm = (a: Piece, b: Piece, monthThenDay: boolean) => {
    tokens.set(a, monthThenDay ? (two(a) ? 'MM' : 'M') : two(a) ? 'DD' : 'D');
    tokens.set(b, monthThenDay ? (two(b) ? 'DD' : 'D') : two(b) ? 'MM' : 'M');
  };
  if (year === 0) {
    tokens.set(rest[0]!, 'YYYY');
    if (monthName) {
      if (rest[1]) tokens.set(rest[1], two(rest[1]) ? 'DD' : 'D');
      rest = rest.slice(2);
    } else {
      if (rest.length < 3) return null;
      dm(rest[1]!, rest[2]!, true);
      rest = rest.slice(3);
    }
  } else if (year > 0 && year <= 2 && !(year === 2 && monthName)) {
    if (monthName) {
      // Nov 18 2026 / 18 Nov 2026
      tokens.set(rest[0]!, two(rest[0]!) ? 'DD' : 'D');
    } else {
      if (year !== 2) return null;
      const [a, b] = [Number(rest[0]!.text), Number(rest[1]!.text)];
      const monthThenDay = a > 12 ? false : b > 12 ? true : mdy;
      dm(rest[0]!, rest[1]!, monthThenDay);
    }
    tokens.set(rest[year]!, 'YYYY');
    rest = rest.slice(year + 1);
  } else if (year < 0) {
    if (monthName) {
      tokens.set(rest[0]!, two(rest[0]!) ? 'DD' : 'D');
      rest = rest.slice(1);
    } else if (rest.length >= 5 && rest.every(two)) {
      // 11_18_14_32_05: month, day, then the time; 26_11_18_14_32_05: the year first
      if (rest.length >= 6 && !hasColon(all)) {
        tokens.set(rest[0]!, 'YY');
        dm(rest[1]!, rest[2]!, true);
        rest = rest.slice(3);
      } else {
        dm(rest[0]!, rest[1]!, true);
        rest = rest.slice(2);
      }
    } else {
      // the hour is the number before the first colon; one number before it is the day, two the
      // day and the month
      const hour = rest.findIndex((n) => all[all.indexOf(n) + 1]?.text.startsWith(':'));
      if (hour === 1) tokens.set(rest[0]!, two(rest[0]!) ? 'DD' : 'D');
      else if (hour === 2) dm(rest[0]!, rest[1]!, mdy);
      else if (hour > 2) return null;
      if (hour > 0) rest = rest.slice(hour);
    }
  } else {
    return null;
  }

  // the time: hours, minutes, seconds, a part of a second; a zone after it
  const zone = zoneAfter(all, rest);
  if (zone) rest = rest.slice(0, zone.index);
  const [h, m, s, f, ...more] = rest;
  if (more.length) return null;
  if (h) tokens.set(h, ampm ? (two(h) ? 'hh' : 'H') : two(h) ? 'HH' : 'H');
  if (m) {
    if (!two(m)) return null;
    tokens.set(m, 'mm');
  }
  if (s) {
    if (!two(s)) return null;
    tokens.set(s, 'ss');
  }
  if (f) tokens.set(f, 'S'.repeat(Math.min(f.text.length, 9)));
  if (zone) for (const [i, p] of zone.pieces.entries()) tokens.set(p, i === 0 ? 'Z' : '');
  if (!h && !tokens.has(nums[0]!)) return null;
  return check(all, tokens, text, day, utc);
}

const hasColon = (all: Piece[]) => all.some((p) => p.kind === 'sep' && p.text.includes(':'));

/** +01:00, -0500 or +01 after the seconds: the pieces it is made of (the sign's included). */
function zoneAfter(all: Piece[], rest: Piece[]): { index: number; pieces: Piece[] } | null {
  for (let i = 2; i < rest.length; i++) {
    const at = all.indexOf(rest[i]!);
    const sign = all[at - 1];
    if (!sign || sign.kind !== 'sep' || !/[+-]$/.test(sign.text)) continue;
    const n = rest[i]!;
    if (n.text.length === 4) return { index: i, pieces: [sign, n] };
    const colon = all[at + 1];
    const minutes = all[at + 2];
    if (n.text.length === 2 && colon?.text === ':' && minutes?.kind === 'num' && minutes.text.length === 2 && rest[i + 1] === minutes) {
      return { index: i, pieces: [sign, n, colon, minutes] };
    }
  }
  return null;
}

/** Write the format, and keep it only if it reads the selection back. */
function check(all: Piece[], tokens: Map<Piece, string>, text: string, day: number, utc: boolean): string | null {
  let out = '';
  for (const p of all) {
    const token = tokens.get(p);
    if (token !== undefined) {
      // a sign that is part of the zone is in its token
      out += p.kind === 'sep' && token === 'Z' ? p.text.replace(/[+-]$/, '') + 'Z' : token;
    } else {
      out += literal(p.text);
    }
  }
  const f = compile(out);
  if (!isFormat(f) || readWith(f as TimeFormat, text, day, utc) === null) return null;
  return out;
}
