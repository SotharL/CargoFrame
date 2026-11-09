// The formatted text box shows its marks as they will look: `**word**` is bold, `> ` starts a
// quote, `- ` a bullet. The text stays the marks (the code that sends it writes them the way each
// service wants, core/connectors/src/richtext.rs, which this reads them like); the box only draws
// them. A mark's `**` are hidden unless the cursor is at it, so it can still be typed over.
//
// Every character of the text is in the box, in order, one line per <div>: a place in the text
// and a place in the box are found from each other by counting.

/** A piece of a line, by where it starts and ends in the whole text. */
export type Piece =
  | { kind: 'text'; s: number; e: number }
  | { kind: 'bold' | 'italic' | 'underline'; s: number; e: number; m: number; inner: Piece[] }
  | { kind: 'code'; s: number; e: number };

export type LineKind = 'plain' | 'quote' | 'bullet' | 'number';

/** A mark is looked for this far ahead and no further (as in richtext.rs). */
const REACH = 2000;
const WORD = /[\p{L}\p{N}]/u;
const isWord = (c: string | undefined) => c !== undefined && WORD.test(c);
const isSpace = (c: string | undefined) => c !== undefined && /\s/.test(c);

function nextTick(s: string, from: number, to: number): number {
  const stop = Math.min(to, from + REACH);
  for (let i = from; i < stop; i++) if (s[i] === '`') return i;
  return -1;
}

function findClose(s: string, from: number, to: number, marker: string): number {
  let i = from;
  while (i < to && i - from <= REACH) {
    if (s[i] === '`') {
      const t = nextTick(s, i + 1, to);
      if (t < 0) return -1;
      i = t + 1;
      continue;
    }
    if (i > from && i + marker.length <= to && s.startsWith(marker, i)) {
      const after = i + marker.length < to ? s[i + marker.length] : undefined;
      const before = s[i - 1];
      const partOfLonger = marker.length === 1 && (after === marker || before === marker);
      if (!partOfLonger && !isSpace(before) && !isWord(after)) return i;
    }
    i += 1;
  }
  return -1;
}

/** The marks of `s` between `from` and `to`, read the way the code that sends them reads them. */
export function parse(s: string, from = 0, to = s.length): Piece[] {
  const out: Piece[] = [];
  let plain = -1;
  const flush = (at: number) => {
    if (plain >= 0 && at > plain) out.push({ kind: 'text', s: plain, e: at });
    plain = -1;
  };
  let i = from;
  while (i < to) {
    const c = s[i];
    const atEdge = i === from || !isWord(s[i - 1]);
    const next = i + 1 < to ? s[i + 1] : undefined;
    if (c === '`') {
      const close = nextTick(s, i + 1, to);
      if (close > i + 1) {
        flush(i);
        out.push({ kind: 'code', s: i, e: close + 1 });
        i = close + 1;
        continue;
      }
    } else if (atEdge && (c === '*' || c === '_') && next === c) {
      const inner = i + 2;
      if (inner < to && !isSpace(s[inner])) {
        const end = findClose(s, inner, to, c + c);
        if (end >= 0) {
          flush(i);
          out.push({ kind: c === '*' ? 'bold' : 'underline', s: i, e: end + 2, m: 2, inner: parse(s, inner, end) });
          i = end + 2;
          continue;
        }
      }
    } else if (atEdge && c === '*' && next !== undefined && !isSpace(next) && next !== '*') {
      const end = findClose(s, i + 1, to, '*');
      if (end >= 0) {
        flush(i);
        out.push({ kind: 'italic', s: i, e: end + 1, m: 1, inner: parse(s, i + 1, end) });
        i = end + 1;
        continue;
      }
    }
    if (plain < 0) plain = i;
    i += 1;
  }
  flush(to);
  return out;
}

/** What a line is, and how long the mark at its start is (with the spaces before it). */
export function classify(line: string): { kind: LineKind; mark: number } {
  const lead = line.length - line.trimStart().length;
  const t = line.slice(lead);
  const hasRest = (n: number) => t.slice(n).trim() !== '';
  if (t.startsWith('> ')) return { kind: 'quote', mark: lead + 2 };
  if ((t.startsWith('- ') || t.startsWith('* ')) && hasRest(2)) return { kind: 'bullet', mark: lead + 2 };
  const digits = /^\d{1,9}(?=[.)] )/.exec(t)?.[0].length ?? 0;
  if (digits > 0 && hasRest(digits + 2)) return { kind: 'number', mark: lead + digits + 2 };
  return { kind: 'plain', mark: 0 };
}

const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
/** Plain text, with each {{variable}} shown as one. */
const withVars = (s: string) => esc(s).replace(/\{\{[^{}\n]{1,80}\}\}/g, (v) => `<span class="var">${v}</span>`);

/** Whether a mark from `s` to `e` is where the selection is (or right next to it). */
const touches = (s: number, e: number, sel: [number, number] | null) => sel !== null && s <= sel[1] && e >= sel[0];

function pieces(text: string, list: Piece[], sel: [number, number] | null): string {
  let out = '';
  for (const p of list) {
    const pos = `data-s="${p.s}" data-e="${p.e}"`;
    if (p.kind === 'text') {
      out += withVars(text.slice(p.s, p.e));
    } else if (p.kind === 'code') {
      const on = touches(p.s, p.e, sel) ? ' on' : '';
      out += `<span class="m code${on}" ${pos}><span class="mk">\`</span><code>${esc(text.slice(p.s + 1, p.e - 1))}</code><span class="mk">\`</span></span>`;
    } else {
      const on = touches(p.s, p.e, sel) ? ' on' : '';
      const m = esc(text.slice(p.s, p.s + p.m));
      out += `<span class="m ${p.kind}${on}" ${pos}><span class="mk">${m}</span>${pieces(text, p.inner, sel)}<span class="mk">${m}</span></span>`;
    }
  }
  return out;
}

/** The box's content for `text`, with the marks at the selection showing their `**`. */
export function toHtml(text: string, sel: [number, number] | null = null): string {
  let at = 0;
  return text
    .split('\n')
    .map((line) => {
      const start = at;
      at += line.length + 1;
      if (line === '') return '<div class="ln"><br></div>';
      const { kind, mark } = classify(line);
      const head = mark > 0 ? `<span class="lm">${esc(line.slice(0, mark))}</span>` : '';
      // the marks are read on the whole text, so their places are places in the whole text
      const body = pieces(text, parse(text, start + mark, start + line.length), sel);
      return `<div class="ln ${kind}">${head}${body}</div>`;
    })
    .join('');
}

/** The box's lines: each <div>, or a run of loose text the browser left outside one. */
function linesOf(root: HTMLElement): Node[][] {
  const out: Node[][] = [];
  let loose: Node[] | null = null;
  for (const n of Array.from(root.childNodes)) {
    const isLine = n instanceof HTMLElement && (n.tagName === 'DIV' || n.tagName === 'P');
    if (isLine) {
      loose = null;
      out.push([n]);
    } else if (n instanceof HTMLElement && n.tagName === 'BR') {
      if (!loose) out.push([]);
      loose = null;
    } else {
      if (!loose) {
        loose = [];
        out.push(loose);
      }
      loose.push(n);
    }
  }
  return out;
}

const lengthOf = (nodes: Node[]) => nodes.reduce((n, x) => n + (x.textContent?.length ?? 0), 0);

/** The text in the box, whatever the browser did to it. */
export function readText(root: HTMLElement): string {
  const lines = linesOf(root);
  if (lines.length === 0) return '';
  return lines.map((l) => l.map((n) => n.textContent ?? '').join('')).join('\n');
}

/** The place in the text of a place in the box. */
export function offsetOf(root: HTMLElement, node: Node, offset: number): number {
  const lines = linesOf(root);
  let before = 0;
  // a place between the box's own children
  if (node === root) {
    const child = root.childNodes[offset];
    for (const l of lines) {
      if (child && l.includes(child)) return before;
      before += lengthOf(l) + 1;
    }
    return Math.max(0, before - 1);
  }
  for (const l of lines) {
    const top = l.find((n) => n === node || n.contains(node));
    if (top) {
      let inLine = lengthOf(l.slice(0, l.indexOf(top)));
      const r = document.createRange();
      r.setStart(top, 0);
      r.setEnd(node, offset);
      inLine += r.toString().length;
      return before + inLine;
    }
    before += lengthOf(l) + 1;
  }
  return before > 0 ? before - 1 : 0;
}

/** The place in the box of a place in the text (the box holding what toHtml made). */
export function pointAt(root: HTMLElement, offset: number): [Node, number] {
  const lines = Array.from(root.children);
  let left = offset;
  for (const [i, line] of lines.entries()) {
    const len = line.textContent?.length ?? 0;
    if (left <= len || i === lines.length - 1) {
      const walk = document.createTreeWalker(line, NodeFilter.SHOW_TEXT);
      let t = walk.nextNode() as Text | null;
      while (t) {
        if (left <= t.length) return [t, left];
        left -= t.length;
        t = walk.nextNode() as Text | null;
      }
      return [line, 0];
    }
    left -= len + 1;
  }
  return [root, 0];
}

/** Where the selection is in the text, if it is in the box. */
export function selectionIn(root: HTMLElement): [number, number] | null {
  const sel = root.ownerDocument.getSelection();
  if (!sel || sel.rangeCount === 0) return null;
  const r = sel.getRangeAt(0);
  if (!root.contains(r.startContainer) || !root.contains(r.endContainer)) return null;
  const a = offsetOf(root, r.startContainer, r.startOffset);
  const b = offsetOf(root, r.endContainer, r.endOffset);
  return a <= b ? [a, b] : [b, a];
}

/** Put the selection at these places in the text. */
export function select(root: HTMLElement, start: number, end: number): void {
  const sel = root.ownerDocument.getSelection();
  if (!sel) return;
  const [n1, o1] = pointAt(root, start);
  const [n2, o2] = start === end ? [n1, o1] : pointAt(root, end);
  const r = root.ownerDocument.createRange();
  r.setStart(n1, o1);
  r.setEnd(n2, o2);
  sel.removeAllRanges();
  sel.addRange(r);
}

/** Show the `**` of the marks at the selection, hide the others: nothing else in the box moves. */
export function showMarksAt(root: HTMLElement, sel: [number, number] | null): void {
  for (const el of Array.from(root.querySelectorAll<HTMLElement>('.m'))) {
    el.classList.toggle('on', touches(Number(el.dataset.s), Number(el.dataset.e), sel));
  }
}

/** What Enter adds: a new line, which goes on with the list or quote the line is in. */
export function newLine(text: string, start: number, end: number): { text: string; at: number } {
  const first = text.lastIndexOf('\n', start - 1) + 1;
  const stop = text.indexOf('\n', start);
  const line = text.slice(first, stop < 0 ? text.length : stop);
  const lead = /^(\s*)(> |[-*] |(\d{1,9})([.)]) )/.exec(line);
  if (lead && start >= first + lead[0].length) {
    // Enter on an empty item ends the list
    if (line.slice(lead[0].length).trim() === '' && end === start) {
      const bare = text.slice(0, first) + text.slice(first + lead[0].length);
      return { text: bare, at: first };
    }
    const mark = lead[3] ? `${lead[1]}${Number(lead[3]) + 1}${lead[4]} ` : lead[0];
    const ins = '\n' + mark;
    return { text: text.slice(0, start) + ins + text.slice(end), at: start + ins.length };
  }
  return { text: text.slice(0, start) + '\n' + text.slice(end), at: start + 1 };
}
