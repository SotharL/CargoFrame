// A value chosen by selecting it in a file: how the selection becomes a way to find it again
// (`Find::Pick`), and that way run on a file's text, as core/rules/src/find.rs runs it. The two
// must agree, so what the editor shows is what a capture finds: the cases are tested on both sides.
import type { RuleFind, RuleWhich } from '../../lib/types';

export type Pick = Extract<RuleFind, { type: 'pick' }>;

/** One place a pick finds a value: on line `line` (from 0), from `s` to `e`. */
export interface Hit {
  line: number;
  s: number;
  e: number;
  value: string;
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

/** Every value `find` gives in `lines`, with where it is. */
export function hits(lines: readonly string[], find: Pick): Hit[] {
  const stop = find.stop ? find.stop : null;
  const out: Hit[] = [];
  lines.forEach((line, n) => {
    const at = find.lines_with ? line.indexOf(find.lines_with) : 0;
    if (at < 0) return;
    let from = find.from_line_start ? 0 : at + find.lines_with.length;
    while (line[from] === ' ') from += 1;
    let to = line.length;
    if (stop !== null) {
      // looked for after the value's first character, so a value that starts with it is not empty
      const cp = line.codePointAt(from);
      const first = cp === undefined ? 0 : cp > 0xffff ? 2 : 1;
      to = line.indexOf(stop, from + first);
      if (to < 0) return;
    }
    let s = from;
    let e = to;
    while (s < e && isSpace(line[s])) s += 1;
    while (e > s && isSpace(line[e - 1])) e -= 1;
    if (e > s) out.push({ line: n, s, e, value: line.slice(s, e) });
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
  const m = /([\p{L}][\p{L}\p{N}_]*[^\s\p{L}\p{N}_]*(?:\s*[^\s\p{L}\p{N}_]+)?)\s*$/u.exec(line.slice(0, s));
  return m ? m[1]! : '';
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

/** The pick a selection suggests: in the lines with the label before it, up to what follows it. */
export function suggest(line: string, s: number, e: number): Pick {
  const label = labelBefore(line, s);
  const next = after(line, e);
  const stop = next === 'space' ? ' ' : next === 'sign' ? line[e]! : null;
  return { type: 'pick', lines_with: label, from_line_start: !label, stop };
}

const q = (t: string) => `“${t}”`;

/** How a pick finds its value, in a few words: "After “LogAI:”, up to “ ”, the most recent line". */
export function describePick(find: Pick, which: RuleWhich): string {
  const start = !find.lines_with
    ? 'Every line, from its start'
    : find.from_line_start
      ? `Lines with ${q(find.lines_with)}, from their start`
      : `After ${q(find.lines_with)}`;
  const stop = !find.stop ? 'to the end of the line' : find.stop === ' ' ? 'up to a space' : `up to ${q(find.stop)}`;
  const kept = which === 'first' ? 'the first line' : which === 'all' ? 'every line' : 'the most recent line';
  return `${start}, ${stop}, ${kept}`;
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
