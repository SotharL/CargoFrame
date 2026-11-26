/**
 * Helpers for the "Text automation" editor: put a snippet where the cursor is, and build a link
 * whose address can carry live data (`{build}`). Plain functions, so they are tested without a screen.
 */

import { msg, t } from '../../lib/i18n/index.svelte';

export type LinkStyle = 'markdown' | 'jira' | 'plain';

export const LINK_STYLES: { value: LinkStyle; label: string; example: string }[] = [
  { value: 'markdown', label: 'Markdown', example: msg('[word](https://…)  Reddit, Claude, Discord, Slack replies') },
  { value: 'jira', label: 'Jira wiki', example: msg('[word|https://…]  Jira descriptions and comments') },
  { value: 'plain', label: msg('Plain text'), example: msg('word (https://…)  anywhere') },
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

/** A web address worth sending: starts with http(s):// or begins with a value (`{tracker}`) that will supply it. */
export function looksLikeAddress(url: string): boolean {
  const u = url.trim();
  return /^https?:\/\/\S+$/i.test(u) || /^\{\{[\w.\- ]+\}\}\S*$/.test(u) || /^\{[\w.-]+\}\S*$/.test(u);
}

/** The variables worth a button of their own, in this order, when the app offers them. */
/** The values most texts use, first in the list. */
export const QUICK_VARIABLES = ['capture.date', 'capture.time', 'app.exe', 'project.name', 'app.title'];

const LABELS: Record<string, string> = {
  'capture.date': msg('Date'),
  'capture.time': msg('Time'),
  'app.exe': msg('Program'),
  'app.path': msg('Program path'),
  'app.title': msg('Window title'),
  'project.name': msg('Project'),
  'project.folder': msg('Project folder'),
  'capture.screenshot': msg('Screenshot'),
  'capture.recording': msg('Recording'),
};
/** The name of a value as the person reads it, in their language. */
export const quickLabel = (name: string): string => (LABELS[name] ? t(LABELS[name]) : name);

/** The pictures a text can show where it names them, and the services that do. */
const MEDIA_VARIABLES = ['capture.screenshot', 'capture.recording'];
export const MEDIA_SERVICES: Record<string, string[]> = {
  'capture.screenshot': ['jira', 'github', 'teams'],
  'capture.recording': ['jira', 'github'],
};
/** A table with the recording and the screenshot side by side, as a bug report shows them. */
export const MEDIA_TABLE = msg('| Repro | Highlight |\n| --- | --- |\n| {capture.recording} | {capture.screenshot} |');

/** The group a value is listed under (in English: show it with t()). */
export function groupOf(name: string): string {
  if (MEDIA_VARIABLES.includes(name)) return msg('In the text');
  if (name.startsWith('app.')) return msg('Program');
  if (name.startsWith('project.')) return msg('Project');
  if (name.startsWith('capture.')) return msg('Capture');
  if (name.startsWith('rule.')) return msg('From files');
  if (name.startsWith('system.')) return msg('This computer');
  if (name.startsWith('cargoframe.')) return msg('CargoFrame (debug mode)');
  return msg('Other');
}
export const GROUPS = ['Program', 'Project', 'Capture', 'From files', 'This computer', 'CargoFrame (debug mode)', 'In the text', 'Other'];
/** The values most texts use, as a group of the list. */
export const MOST_USED = msg('Most used');

/** Whether a value matches what was typed in the search: its name, its label or what it is. */
export function matches(q: string, ...said: string[]): boolean {
  const words = q.toLowerCase().split(/\s+/).filter(Boolean);
  const hay = said.join(' ').toLowerCase();
  return words.every((w) => hay.includes(w));
}
