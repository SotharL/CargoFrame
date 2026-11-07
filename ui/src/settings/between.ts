// "Text before, the value, text after" for a text or log file, kept as the instruction the
// reader of text files already understands (`after: Version:` or `between: build= | ;`), so a
// rule saved this way is an ordinary rule and nothing needs converting.

export interface Between {
  before: string;
  after: string;
}

/** The two texts of an `after:` or `between:` instruction; empty for no instruction yet. */
export function readBetween(query: string): Between | null {
  const q = query.trim();
  if (!q) return { before: '', after: '' };
  const [, key = '', arg = ''] = /^(\w+)\s*:\s*(.*)$/s.exec(q) ?? [];
  const word = key.toLowerCase();
  if (word === 'after') return { before: arg.trim(), after: '' };
  if (word === 'between') {
    const bar = arg.indexOf('|');
    return bar < 0 ? null : { before: arg.slice(0, bar).trim(), after: arg.slice(bar + 1).trim() };
  }
  return null;
}

/** The instruction for two texts. Without the text before there is nothing to look for yet. */
export function writeBetween({ before, after }: Between): string {
  const b = before.trim();
  const a = after.trim();
  if (!b) return '';
  return a ? `between: ${b} | ${a}` : `after: ${b}`;
}

/** Whether a query is one of the text reader's own instructions (so it is worth keeping). */
export const isTextQuery = (query: string): boolean =>
  /^\s*(line|lines|head|tail|contains|after|between|level)\s*:/i.test(query);
