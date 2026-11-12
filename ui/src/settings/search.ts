// The settings search: what each page has, by the words a person may look for. A finding names
// its page and what to show there: a section (by its title, as the page shows it), or a thing
// with a `data-tour` name. search.test.ts fails when one points at something that no longer
// exists, so the search cannot silently go stale. To add a setting, add a line here.
import { PAGES, type PageId } from './nav';

export interface Finding {
  page: PageId;
  /** What it is, as shown in the results. */
  title: string;
  /** Other words it is found by. */
  words?: string;
  /** The section to scroll to: its title (an `h2` of the page). */
  section?: string;
  /** Or the thing to scroll to: its `data-tour` name. */
  target?: string;
  /** The part of the page (a tab), as the page's address names it. */
  arg?: string;
}

export const FINDINGS: Finding[] = [
  // Setup
  { page: 'setup', title: 'This computer', section: 'This computer', words: 'ffmpeg xdotool tesseract claude app installed programs tools check missing' },
  { page: 'setup', title: 'Install what is missing', section: 'Install what is missing', words: 'winget download install ffmpeg tesseract' },
  {
    page: 'setup', title: 'Connections', section: 'Connections', arg: 'connections',
    words: 'services accounts token sign in jira github google drive sheets slack discord teams reddit claude x twitter bluesky instagram custom api webhook',
  },
  { page: 'setup', title: 'Add a connection', section: 'Add a connection', arg: 'connections', words: 'new service account jira github google slack discord teams reddit claude x bluesky instagram api' },
  // Layouts
  { page: 'layouts', title: 'Ready-made layouts', target: 'layouts.templates', words: 'templates jira bug github issue start new layout' },
  {
    page: 'layouts', title: 'Blocks of a layout', target: 'layouts.save',
    words: 'grid columns rows window screenshot video recording text dropdown files tags labels log viewer sharing button block',
  },
  { page: 'layouts', title: 'Keys pressed on a recording (Video block)', words: 'keyboard keystrokes shortcuts typing overlay video block' },
  { page: 'layouts', title: 'Time on a recording (Video block)', words: 'clock timestamp date hour overlay video block' },
  { page: 'layouts', title: 'Log lines by time (Log viewer block)', words: 'timestamp time format during recording utc between times filter log viewer' },
  { page: 'layouts', title: 'One line high (Text block)', words: 'text block height single line size' },
  { page: 'layouts', title: 'Formatting buttons (Text block)', words: 'bold italic code lists tables markdown text block' },
  { page: 'layouts', title: 'Tags block', words: 'labels hashtags tag container jira github' },
  { page: 'layouts', title: 'Log viewer block', words: 'log parser search filter lines copy file preview' },
  { page: 'layouts', title: 'Where each block is sent', target: 'props.sent_in', words: 'field mapping sharing button sent in jira field' },
  // Triggers
  { page: 'triggers', title: 'Add a trigger', target: 'triggers.add', words: 'shortcut hotkey keyboard key combination capture screenshot record' },
  { page: 'triggers', title: 'Shortcut', target: 'triggers.shortcut', words: 'hotkey keyboard keys combination ctrl alt shift' },
  { page: 'triggers', title: 'What a trigger makes', target: 'triggers.makes', words: 'screenshot recording video layout opens' },
  { page: 'triggers', title: 'Shortcuts on this desktop', target: 'triggers.desktop', words: 'wayland gnome sway hyprland kde desktop shortcuts' },
  // Data & fields
  { page: 'rules', title: 'Read from files', target: 'rules.add', arg: 'files', words: 'data rules values file version config log json xml ini yaml project folder read' },
  { page: 'rules', title: 'Time format of a log', target: 'rules.log_time', arg: 'files', words: 'log viewer timestamp time format YYYY MM DD HH mm ss lines game log' },
  { page: 'rules', title: 'Values made from other values', target: 'rules.add', arg: 'files', words: 'combine combination build id conditions if found missing choice' },
  { page: 'rules', title: 'Fill fields', target: 'fields.add', arg: 'fields', words: 'jira labels priority component assignee field service fill mapping extra fields' },
  // Recording
  { page: 'video', title: 'ffmpeg', section: 'ffmpeg', words: 'program path video tool find' },
  { page: 'video', title: 'Capture and compression', section: 'Capture and compression', words: 'quality crf fps frame rate height size codec encoder hardware seconds kept length sound audio microphone' },
  { page: 'video', title: 'Continuous recording', section: 'Continuous recording', target: 'video.continuous', words: 'always record hold shortcut last seconds buffer' },
  { page: 'video', title: 'Follow a window that is moved', section: 'A window that is moved', target: 'video.follow-window', words: 'moving window drag follow crop' },
  { page: 'video', title: 'While recording', section: 'While recording', target: 'video.sign', words: 'sign outline red dot blinking indicator' },
  { page: 'video', title: 'Try it on this PC', section: 'Try it on this PC', words: 'test record trial check' },
  // General
  { page: 'general', title: 'Appearance', section: 'Appearance', words: 'theme dark light colour color accent look' },
  { page: 'general', title: 'Startup', section: 'Startup', words: 'start with the computer login autostart tray boot' },
  { page: 'general', title: 'Notifications', section: 'Notifications', words: 'notices toasts messages popups' },
  { page: 'general', title: 'Windows kept ready', section: 'Windows kept ready', words: 'memory warm fast open speed ram' },
  { page: 'general', title: 'When sharing fails', section: 'When sharing fails', words: 'retry fallback zip save locally folder offline failed' },
  // Your data
  { page: 'data', title: 'Your data', section: 'Your data', words: 'files kept folders delete clear cache logs captures reset start again privacy' },
  { page: 'data', title: 'Export and import', section: 'Export and import', words: 'backup move another computer copy settings layouts restore' },
  // About
  { page: 'about', title: 'Updates', section: 'Updates', words: 'version check update download install new release' },
  { page: 'about', title: 'Speed and memory', section: 'Speed and memory', words: 'diagnostics performance timing fast slow' },
];

/** Lower case without accents, so "Créer" is found by "creer". */
export const fold = (s: string) => s.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase();

/** The words looked for: what is typed, split on spaces. */
export const wordsOf = (query: string) => fold(query).split(/\s+/).filter(Boolean);

/** Does the text have every word (or the start of one)? */
export const hasAll = (text: string, words: readonly string[]) => {
  const t = fold(text);
  return words.every((w) => t.includes(w));
};

/** The findings with every word typed, in their title, page or other words; titles first. */
export function search(query: string): Finding[] {
  const words = wordsOf(query);
  if (words.length === 0) return [];
  const pageTitle = (id: PageId) => PAGES.find((p) => p.id === id)?.title ?? '';
  const scored = FINDINGS.map((f, i) => {
    const all = `${f.title} ${pageTitle(f.page)} ${f.words ?? ''}`;
    if (!hasAll(all, words)) return null;
    return { f, i, best: hasAll(f.title, words) ? 0 : 1 };
  }).filter((x): x is { f: Finding; i: number; best: number } => x !== null);
  return scored.sort((a, b) => a.best - b.best || a.i - b.i).map((x) => x.f);
}

/** Pages whose own name or description has the words, beyond their findings. */
export function pagesNamed(query: string): PageId[] {
  const words = wordsOf(query);
  if (words.length === 0) return [];
  return PAGES.filter((p) => hasAll(`${p.title} ${p.blurb}`, words)).map((p) => p.id);
}

/** Where a finding is on its page, once the page shows: its section, or its `data-tour` thing. */
export function placeOf(f: Finding, root: ParentNode = document): HTMLElement | null {
  if (f.target) {
    const el = root.querySelector<HTMLElement>(`[data-tour="${f.target}"]`);
    if (el) return el;
  }
  if (f.section) {
    for (const h of root.querySelectorAll<HTMLElement>('h2')) {
      if (h.textContent?.trim() === f.section) return (h.closest('section') as HTMLElement | null) ?? h;
    }
  }
  return null;
}
