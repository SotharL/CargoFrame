// Formatting for a plain text box: which buttons to offer (what the services the text goes to can
// show) and what each button does to the text. The text is written with a few marks (`**bold**`,
// `*italic*`, `__underline__`, `` `code` ``, and `> `, `- `, `1. ` at the start of a line); the
// code that sends it writes them the way each service wants (core/connectors/src/richtext.rs).
import type { Block, ServiceKind, SessionView } from '../../lib/types';

export type Mark = 'bold' | 'italic' | 'underline' | 'code' | 'quote' | 'bullets' | 'numbers';

export const MARKS: { mark: Mark; label: string; text: string; hint: string }[] = [
  { mark: 'bold', label: 'Bold', text: 'B', hint: 'Bold (Ctrl+B)' },
  { mark: 'italic', label: 'Italic', text: 'I', hint: 'Italic (Ctrl+I)' },
  { mark: 'underline', label: 'Underline', text: 'U', hint: 'Underline (Ctrl+U)' },
  { mark: 'code', label: 'Code', text: '</>', hint: 'Code' },
  { mark: 'quote', label: 'Quote', text: '❝', hint: 'Quote' },
  { mark: 'bullets', label: 'Bulleted list', text: '•', hint: 'Bulleted list' },
  { mark: 'numbers', label: 'Numbered list', text: '1.', hint: 'Numbered list' },
];

const EVERY = MARKS.map((m) => m.mark);
const NO_UNDERLINE = EVERY.filter((m) => m !== 'underline');

/**
 * The field of each service that takes a formatted body, and what it can show of the marks (the
 * same table as the converter in core/connectors/src/richtext.rs). Services not listed here
 * (Claude, Google, a custom API) take plain text.
 */
const BODY: Partial<Record<ServiceKind, { field: string; marks: Mark[] }>> = {
  jira: { field: 'description', marks: EVERY },
  discord: { field: 'message', marks: EVERY },
  teams: { field: 'message', marks: EVERY },
  slack: { field: 'message', marks: NO_UNDERLINE },
  reddit: { field: 'text', marks: NO_UNDERLINE },
};

type TextBlock = Extract<Block, { type: 'text' }>;

/**
 * The buttons to offer above a text box: the marks every service it is sent to can show. A box
 * that no button sends as a message body (or that is not plain text, or has formatting switched
 * off) has none. `services` says which ones the buttons are for.
 */
export function marksFor(view: SessionView, block: TextBlock): { marks: Mark[]; services: ServiceKind[] } {
  const none = { marks: [], services: [] };
  if (block.role !== 'plain' || block.formatting === false) return none;
  const kinds: ServiceKind[] = [];
  let shared: Mark[] | null = null;
  for (const b of view.layout?.blocks ?? []) {
    if (b.type !== 'sharing_button') continue;
    const kind = view.sharing[b.id]?.kind;
    const body = kind ? BODY[kind] : undefined;
    if (!kind || !body) continue;
    const sends = b.mappings.some((m) => m.field === body.field && m.source.kind === 'block' && m.source.value === block.id);
    if (!sends) continue;
    if (!kinds.includes(kind)) kinds.push(kind);
    shared = shared === null ? [...body.marks] : shared.filter((m) => body.marks.includes(m));
  }
  return shared && shared.length > 0 ? { marks: shared, services: kinds } : none;
}

export interface Edit {
  text: string;
  /** The selection after the edit. */
  start: number;
  end: number;
}

const WRAP: Partial<Record<Mark, string>> = { bold: '**', italic: '*', underline: '__', code: '`' };
const LINE: Partial<Record<Mark, RegExp>> = {
  quote: /^> /,
  bullets: /^[-*] /,
  numbers: /^\d+[.)] /,
};
const ANY_LINE = /^(?:> |[-*] |\d+[.)] )/;

/** Wrap the selection in a mark, or take the mark off when it is already there. */
function wrap(text: string, start: number, end: number, m: string): Edit {
  const inside = text.slice(start, end);
  // the marks just outside the selection
  if (text.slice(Math.max(0, start - m.length), start) === m && text.slice(end, end + m.length) === m) {
    return {
      text: text.slice(0, start - m.length) + inside + text.slice(end + m.length),
      start: start - m.length,
      end: end - m.length,
    };
  }
  // the marks inside the selection
  if (inside.length >= 2 * m.length + 1 && inside.startsWith(m) && inside.endsWith(m)) {
    const bare = inside.slice(m.length, inside.length - m.length);
    return { text: text.slice(0, start) + bare + text.slice(end), start, end: start + bare.length };
  }
  if (start === end) {
    // nothing selected: the marks go in as a pair, with the cursor between them
    return { text: text.slice(0, start) + m + m + text.slice(end), start: start + m.length, end: start + m.length };
  }
  return { text: text.slice(0, start) + m + inside + m + text.slice(end), start: start + m.length, end: end + m.length };
}

/** Put a list or quote mark on every line the selection touches, or take it off if all have it. */
function lines(text: string, start: number, end: number, mark: Mark): Edit {
  const first = text.lastIndexOf('\n', start - 1) + 1;
  const stop = text.indexOf('\n', end);
  const last = stop < 0 ? text.length : stop;
  const block = text.slice(first, last).split('\n');
  const mine = LINE[mark]!;
  const off = block.every((l) => mine.test(l));
  let n = 0;
  const out = block.map((l) => {
    const bare = l.replace(ANY_LINE, '');
    if (off) return bare;
    n += 1;
    return (mark === 'quote' ? '> ' : mark === 'bullets' ? '- ' : `${n}. `) + bare;
  });
  const joined = out.join('\n');
  return { text: text.slice(0, first) + joined + text.slice(last), start: first, end: first + joined.length };
}

/** What pressing a button does to the text, given the selection. */
export function applyMark(text: string, start: number, end: number, mark: Mark): Edit {
  const m = WRAP[mark];
  return m ? wrap(text, start, end, m) : lines(text, start, end, mark);
}
