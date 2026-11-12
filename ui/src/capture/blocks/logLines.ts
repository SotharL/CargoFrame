// The lines of a log a viewer shows: numbered from 1, and only the ones with every word of the
// search in them when there is one (case aside).

export interface LogLine {
  /** Its number in the text shown, from 1. */
  n: number;
  text: string;
}

/** Every line of a text, numbered. A text that ends with a new line has no empty last line. */
export function splitLines(text: string): LogLine[] {
  const all = text.replace(/\r\n?/g, '\n').split('\n');
  if (all.length > 1 && all[all.length - 1] === '') all.pop();
  return all.map((t, i) => ({ n: i + 1, text: t }));
}

/** The lines with every word of `query` in them; all of them for an empty query. */
export function matching(lines: LogLine[], query: string): LogLine[] {
  const words = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return lines;
  return lines.filter((l) => {
    const lower = l.text.toLowerCase();
    return words.every((w) => lower.includes(w));
  });
}
