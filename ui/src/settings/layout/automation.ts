/**
 * Helpers for the "Text automation" editor: put a snippet where the cursor is, and build a link
 * whose address can carry live data (`{{rule.build}}`). Plain functions, so they are tested without a screen.
 */

export type LinkStyle = 'markdown' | 'jira' | 'plain';

export const LINK_STYLES: { value: LinkStyle; label: string; example: string }[] = [
  { value: 'markdown', label: 'Markdown', example: '[word](https://…)  Reddit, Claude, Discord, Slack replies' },
  { value: 'jira', label: 'Jira wiki', example: '[word|https://…]  Jira descriptions and comments' },
  { value: 'plain', label: 'Plain text', example: 'word (https://…)  anywhere' },
];

/** Replace the selection with `snippet`; the caret goes right after it. */
export function insertAt(text: string, start: number, end: number, snippet: string): { text: string; caret: number } {
  const a = Math.max(0, Math.min(start, text.length));
  const b = Math.max(a, Math.min(end, text.length));
  return { text: text.slice(0, a) + snippet + text.slice(b), caret: a + snippet.length };
}

/** `[word](url)`, `[word|url]` or `word (url)`. With no word, just the address. */
export function makeLink(style: LinkStyle, word: string, url: string): string {
  const w = word.trim();
  const u = url.trim();
  if (!u) return w;
  if (!w) return u;
  switch (style) {
    case 'markdown':
      return `[${w}](${u})`;
    case 'jira':
      return `[${w}|${u}]`;
    default:
      return `${w} (${u})`;
  }
}

/** A web address worth sending: starts with http(s):// or begins with a `{{variable}}` that will supply it. */
export function looksLikeAddress(url: string): boolean {
  const u = url.trim();
  return /^https?:\/\/\S+$/i.test(u) || /^\{\{[\w.\- ]+\}\}\S*$/.test(u);
}

/** The variables worth a button of their own, in this order, when the app offers them. */
export const QUICK_VARIABLES = ['capture.date', 'project.name', 'app.exe'];

export const quickLabel = (name: string): string =>
  ({ 'capture.date': 'Date', 'capture.time': 'Time', 'project.name': 'Project', 'app.exe': 'App', 'app.title': 'Window title' })[name] ?? name;
