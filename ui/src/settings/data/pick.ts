// A value chosen by selecting it in a file: how the selection becomes a way to find it again
// (`Find::Pick`), and that way run on a file's text, as core/rules/src/find.rs runs it. The two
// must agree, so what the editor shows is what a capture finds: the cases are tested on both sides.
import type { RuleFind, RuleWhich } from '../../lib/types';
import { t } from '../../lib/i18n/index.svelte';

export type Pick = Extract<RuleFind, { type: 'pick' }>;

/** Where a part is: on line `line` (from 0), from `s` to `e`. */
export interface Span {
  line: number;
  s: number;
  e: number;
}

/** One place a pick finds a value: its first part (`line`, `s`, `e`), every part with the next
 *  ones (`parts`), and the value they make, joined. */
export interface Hit extends Span {
  value: string;
  parts: Span[];
}

/** The lines of a text, as Rust's `str::lines` gives them. */
export function linesOf(text: string): string[] {
  const lines = text.split('\n').map((l) => (l.endsWith('\r') ? l.slice(0, -1) : l));
  if (lines.length > 0 && lines[lines.length - 1] === '' && text.endsWith('\n')) lines.pop();
  return lines;
}

const isWord = (c: string | undefined) => !!c && /[\p{L}\p{N}_]/u.test(c);
const isSign = (c: string | undefined) => !!c && !isWord(c) && !/\s/.test(c);
const isSpace = (c: string | undefined) => !!c && /\s/.test(c);

type Part = { lines_with: string; from_line_start?: boolean; stop?: string | null };

/** Where one part is on `line`, looking from `at` (as `part_in` in core/rules/src/find.rs). */
function partIn(line: string, at: number, p: Part): { s: number; e: number } | null {
  const k = p.lines_with ? line.indexOf(p.lines_with, at) : at;
  if (k < 0) return null;
  let from = p.from_line_start ? at : k + p.lines_with.length;
  while (line[from] === ' ') from += 1;
  let to = line.length;
  if (p.stop) {
    // looked for after the value's first character, so a value that starts with it is not empty
    const cp = line.codePointAt(from);
    const first = cp === undefined ? 0 : cp > 0xffff ? 2 : 1;
    to = line.indexOf(p.stop, from + first);
    if (to < 0) return null;
  }
  let s = from;
  let e = to;
  while (s < e && isSpace(line[s])) s += 1;
  while (e > s && isSpace(line[e - 1])) e -= 1;
  return e > s ? { s, e } : null;
}

/** Every value `find` gives in `lines`, with where it is: the first part on each line it looks
 *  in, then each next part after the one before (later on that line when it has a text to come
 *  after, else on a line below). A next part not found is left out. */
export function hits(lines: readonly string[], find: Pick): Hit[] {
  const out: Hit[] = [];
  lines.forEach((line, n) => {
    const first = partIn(line, 0, find);
    if (!first) return;
    const parts: Span[] = [{ line: n, ...first }];
    let value = line.slice(first.s, first.e);
    let at = { line: n, e: first.e };
    for (const next of find.then ?? []) {
      const sameLine = !!next.lines_with && !next.from_line_start;
      for (let k = sameLine ? at.line : at.line + 1; k < lines.length; k++) {
        const p = partIn(lines[k]!, k === at.line ? at.e : 0, next);
        if (!p) continue;
        parts.push({ line: k, ...p });
        value += next.joined_by + lines[k]!.slice(p.s, p.e);
        at = { line: k, e: p.e };
        break;
      }
    }
    out.push({ line: n, s: first.s, e: first.e, value, parts });
  });
  return out;
}

/** The places kept: the first, the last (the most recent line of a log) or all. */
export function choose(found: readonly Hit[], which: RuleWhich): Hit[] {
  if (found.length === 0) return [];
  if (which === 'first') return [found[0]!];
  if (which === 'last') return [found[found.length - 1]!];
  return [...found];
}

/** What a file gives: the places kept, joined like a capture joins them. */
export const valueOf = (kept: readonly Hit[]) => kept.map((h) => h.value).join(', ');

/**
 * What a mouse selection on a line takes, from `s` to `e` in it: a click (`s === e`) takes the
 * word under it (letters, digits, _), a drag takes what was dragged over with a word cut at either
 * end taken whole. `signs`: a click on a sign takes that sign (to choose a text to stop at).
 */
export function snap(line: string, s: number, e: number, signs = false): { s: number; e: number } | null {
  if (e < s) [s, e] = [e, s];
  s = Math.max(0, Math.min(s, line.length));
  e = Math.max(0, Math.min(e, line.length));
  if (s === e) {
    if (!isWord(line[s]) && isWord(line[s - 1])) s -= 1;
    if (!isWord(line[s])) return signs && isSign(line[s]) ? { s, e: s + 1 } : null;
    e = s + 1;
  }
  while (s > 0 && isWord(line[s - 1]) && isWord(line[s])) s -= 1;
  while (e < line.length && isWord(line[e - 1]) && isWord(line[e])) e += 1;
  while (s < e && isSpace(line[s])) s += 1;
  while (e > s && isSpace(line[e - 1])) e -= 1;
  return e > s ? { s, e } : null;
}

/**
 * The label just before a selection, signs included: "LogAI:" in "[…]LogAI: Spawned…", and
 * `version": "` in `"version": "1.2"` (a quote right before the value stays, spaces do not).
 */
export function labelBefore(line: string, s: number): string {
  const before = line.slice(0, s);
  const m = /([\p{L}][\p{L}\p{N}_]*[^\s\p{L}\p{N}_]*(?:\s*[^\s\p{L}\p{N}_]+)?)\s*$/u.exec(before);
  if (m) return m[1]!;
  // a label with no letter of its own ("#0:1:") goes with the word before it: "Stream #0:1:"
  const w = /([\p{L}][^\s]*\s+[^\s\p{L}]+)\s*$/u.exec(before);
  return w ? w[1]! : '';
}

/** The first sign after `e`: where a value like "Harbor_P?listen" stops. */
export function signAfter(line: string, e: number): string {
  for (let k = e; k < line.length; k += 1) if (isSign(line[k])) return line[k]!;
  return '';
}

/** What comes right after the selection: a sign, a space or the end of the line. */
export function after(line: string, e: number): 'sign' | 'space' | 'end' {
  const c = line[e];
  return c === undefined ? 'end' : isSpace(c) ? 'space' : isSign(c) ? 'sign' : 'end';
}

/** The pick a selection suggests: in the lines with the label before it, up to what follows it.
 *  The stop is the shortest text right after the selection that is not in it too (a space, or
 *  " on" when the selection is "12 crew"), so the value found again is the one selected. */
export function suggest(line: string, s: number, e: number): Pick {
  const label = labelBefore(line, s);
  return { type: 'pick', lines_with: label, from_line_start: !label, stop: stopAfter(line, s, e) };
}

function stopAfter(line: string, s: number, e: number): string | null {
  if (after(line, e) === 'end') return null;
  const cp = line.codePointAt(s);
  const first = cp === undefined ? 0 : cp > 0xffff ? 2 : 1;
  for (let k = 1; k <= 16 && e + k <= line.length; k++) {
    const stop = line.slice(e, e + k);
    if (stop.trim() === '' && k > 1) continue;
    if (line.indexOf(stop, s + first) !== e) continue;
    // a word cut short is taken whole: " on", not " o"
    let end = e + k;
    while (end < line.length && isWord(line[end]) && isWord(line[end - 1])) end += 1;
    return line.slice(e, end);
  }
  return null;
}

const q = (t: string) => `“${t}”`;

/** How a pick finds its value, in a few words: "After “LogAI:”, up to “ ”, the most recent line". */
export function describePick(find: Pick, which: RuleWhich): string {
  const start = !find.lines_with
    ? t('Every line, from its start')
    : find.from_line_start
      ? t('Lines with {q}, from their start', { q: q(find.lines_with) })
      : t('After {q}', { q: q(find.lines_with) });
  const stop = !find.stop ? t('to the end of the line') : find.stop === ' ' ? t('up to a space') : t('up to {text}', { text: q(find.stop) });
  const kept = which === 'first' ? t('the first line') : which === 'all' ? t('every line') : t('the most recent line');
  const more = find.then?.length ?? 0;
  const then = more === 0 ? '' : ' ' + (more === 1 ? t('+ a next part') : t('+ {n} next parts', { n: more }));
  return `${start}, ${stop}, ${kept}${then}`;
}

/** Every place `query` is in `lines`, case ignored, like Ctrl+F. */
export function search(lines: readonly string[], query: string): { line: number; s: number; e: number }[] {
  const out: { line: number; s: number; e: number }[] = [];
  const needle = query.toLowerCase();
  if (!needle) return out;
  lines.forEach((l, line) => {
    const low = l.toLowerCase();
    for (let k = low.indexOf(needle); k >= 0; k = low.indexOf(needle, k + 1)) out.push({ line, s: k, e: k + needle.length });
  });
  return out;
}
