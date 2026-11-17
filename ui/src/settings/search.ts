// The settings search: what each page has, by the words a person may look for. A finding names
// its page and what to show there: a section (by its title, as the page shows it), or a thing
// with a `data-tour` name. search.test.ts fails when one points at something that no longer
// exists, so the search cannot silently go stale. To add a setting, add a line here.
import { PAGES, type PageId } from './nav';
import { msg, t, translated } from '../lib/i18n/index.svelte';

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

/** The findings with English titles; FINDINGS reads them in the language in use. */
const RAW: Finding[] = [
  // Setup
  { page: 'setup', title: msg('This computer'), section: 'This computer', words: msg('ffmpeg xdotool tesseract claude app installed programs tools check missing') },
  { page: 'setup', title: msg('Install what is missing'), section: 'Install what is missing', words: msg('winget download install ffmpeg tesseract') },
  {
    page: 'setup', title: msg('Connections'), section: 'Connections', arg: 'connections',
    words: msg('services accounts token sign in jira github google drive sheets slack discord teams reddit claude x twitter bluesky instagram custom api webhook'),
  },
  { page: 'setup', title: msg('Add a connection'), section: 'Add a connection', arg: 'connections', words: msg('new service account jira github google slack discord teams reddit claude x bluesky instagram api') },
  // Layouts
  { page: 'layouts', title: msg('Ready-made layouts'), target: 'layouts.templates', words: msg('templates jira bug github issue start new layout') },
  {
    page: 'layouts', title: msg('Blocks of a layout'), target: 'layouts.save',
    words: msg('grid columns rows window screenshot video recording text dropdown files tags labels log viewer tabs sharing button block'),
  },
  { page: 'layouts', title: msg('Keys pressed on a recording (Video block)'), words: msg('keyboard keystrokes shortcuts typing overlay video block') },
  { page: 'layouts', title: msg('Time on a recording (Video block)'), words: msg('clock timestamp date hour overlay video block') },
  { page: 'layouts', title: msg('Log lines by time (Log viewer block)'), words: msg('timestamp time format during recording utc between times filter log viewer') },
  { page: 'layouts', title: msg('One line high (Text block)'), words: msg('text block height single line size') },
  { page: 'layouts', title: msg('Formatting buttons (Text block)'), words: msg('bold italic code lists tables markdown text block') },
  { page: 'layouts', title: msg('Tags block'), words: msg('labels hashtags tag container jira github') },
  { page: 'layouts', title: msg('Log viewer block'), words: msg('log parser search filter lines copy file preview') },
  { page: 'layouts', title: msg('Tabs block'), words: msg('tabs binder pages sections group blocks tab names side') },
  { page: 'layouts', title: msg('Where each block is sent'), target: 'props.sent_in', words: msg('field mapping sharing button sent in jira field') },
  // Triggers
  { page: 'triggers', title: msg('Add a trigger'), target: 'triggers.add', words: msg('shortcut hotkey keyboard key combination capture screenshot record') },
  { page: 'triggers', title: msg('Shortcut'), target: 'triggers.shortcut', words: msg('hotkey keyboard keys combination ctrl alt shift') },
  { page: 'triggers', title: msg('What a trigger makes'), target: 'triggers.makes', words: msg('screenshot recording video layout opens') },
  { page: 'triggers', title: msg('Shortcuts on this desktop'), target: 'triggers.desktop', words: msg('wayland gnome sway hyprland kde desktop shortcuts') },
  // Data & fields
  { page: 'rules', title: msg('New value: select it in a file'), target: 'data.new', arg: 'values', words: msg('data rules values file log select pick map version token name {value} read find') },
  { page: 'rules', title: msg('Files read in a capture'), target: 'rules.add', arg: 'files', words: msg('data rules file version config log json xml ini yaml project folder add to files newest several') },
  { page: 'rules', title: msg('Time format of a log'), target: 'rules.add', arg: 'files', words: msg('log viewer timestamp time format YYYY MM DD HH mm ss lines game log') },
  { page: 'rules', title: msg('Values made from other values'), target: 'data.combined', arg: 'values', words: msg('combine combination build id conditions if found missing choice') },
  { page: 'layouts', title: msg('Fill fields'), target: 'fields.add', arg: 'fields', words: msg('jira labels priority component assignee field service fill mapping extra fields') },
  // Recording
  { page: 'video', title: msg('ffmpeg'), section: 'ffmpeg', words: msg('program path video tool find') },
  { page: 'video', title: msg('Capture and compression'), section: 'Capture and compression', words: msg('quality crf fps frame rate height size codec encoder hardware seconds kept length sound audio microphone') },
  { page: 'video', title: msg('Continuous recording'), section: 'Continuous recording', target: 'video.continuous', words: msg('always record hold shortcut last seconds buffer') },
  { page: 'video', title: msg('Follow a window that is moved'), section: 'A window that is moved', target: 'video.follow-window', words: msg('moving window drag follow crop') },
  { page: 'video', title: msg('While recording'), section: 'While recording', target: 'video.sign', words: msg('sign outline red dot blinking indicator') },
  { page: 'video', title: msg('Try it on this PC'), section: 'Try it on this PC', words: msg('test record trial check') },
  // General
  { page: 'general', title: msg('Appearance'), section: 'Appearance', words: msg('theme dark light colour color accent look') },
  { page: 'general', title: msg('Language'), section: 'Language', words: msg('language translation translated english french spanish italian german locale') },
  { page: 'general', title: msg('Startup'), section: 'Startup', words: msg('start with the computer login autostart tray boot') },
  { page: 'general', title: msg('Notifications'), section: 'Notifications', words: msg('notices toasts messages popups') },
  { page: 'general', title: msg('Windows kept ready'), section: 'Windows kept ready', words: msg('memory warm fast open speed ram') },
  { page: 'general', title: msg('When sharing fails'), section: 'When sharing fails', words: msg('retry fallback zip save locally folder offline failed') },
  // Your data
  { page: 'data', title: msg('Your data'), section: 'Your data', words: msg('files kept folders delete clear cache logs captures reset start again privacy') },
  { page: 'data', title: msg('Export and import'), section: 'Export and import', words: msg('backup move another computer copy settings layouts restore') },
  // About
  { page: 'about', title: msg('Updates'), section: 'Updates', words: msg('version check update download install new release') },
  { page: 'about', title: msg('Speed and memory'), section: 'Speed and memory', words: msg('diagnostics performance timing fast slow') },
];
export const FINDINGS: Finding[] = translated(RAW, new Set(['title', 'words']));


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
    // found by the words of the language in use, and by the English ones
    const all = `${f.title} ${RAW[i]!.title} ${pageTitle(f.page)} ${f.words ?? ''} ${RAW[i]!.words ?? ''}`;
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
      const title = h.textContent?.trim();
      if (title === f.section || title === t(f.section)) return (h.closest('section') as HTMLElement | null) ?? h;
    }
  }
  return null;
}
