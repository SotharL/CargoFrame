import { msg, translated } from '../lib/i18n/index.svelte';
export type PageId =
  | 'setup'
  | 'layouts'
  | 'triggers'
  | 'rules'
  | 'video'
  | 'general'
  | 'data'
  | 'about';

/** The sidebar is in three groups: getting going, what a capture does, and the program itself. */
export type PageGroup = 'start' | 'capture' | 'app';

export interface PageMeta {
  id: PageId;
  title: string;
  blurb: string;
  group: PageGroup;
}

export const GROUP_TITLES: Record<PageGroup, string> = translated({
  start: msg('Get started'),
  capture: msg('Captures'),
  app: msg('CargoFrame'),
}, 'all');

/** Order matches the settings window's sidebar: the order a new person sets things up in. */
export const PAGES: PageMeta[] = translated([
  { id: 'setup', group: 'start', title: msg('Setup'), blurb: msg('What CargoFrame uses on this computer, then the services you share to.') },
  { id: 'layouts', group: 'capture', title: msg('Layouts'), blurb: msg('Build the window each trigger opens, block by block, and what fills the fields of a service.') },
  { id: 'triggers', group: 'capture', title: msg('Triggers'), blurb: msg('Shortcuts that capture your screen and open a layout.') },
  { id: 'rules', group: 'capture', title: msg('Data & fields'), blurb: msg('Values read from the files of a capture, like the map in a log or a version, for layouts to write in titles, tags and fields.') },
  { id: 'video', group: 'capture', title: msg('Recording'), blurb: msg('How videos are recorded on this PC: where ffmpeg is, how it captures and compresses, and a short test.') },
  { id: 'general', group: 'app', title: msg('General'), blurb: msg('How CargoFrame looks, starts, tells you things, keeps windows ready, and what it does when a share fails.') },
  { id: 'data', group: 'app', title: msg('Your data'), blurb: msg('What CargoFrame keeps on this computer, a backup of your layouts, and a way to start again.') },
  { id: 'about', group: 'app', title: msg('About'), blurb: msg('The version, updates, and how fast CargoFrame has been on this computer.') },
]);

/** The page a new window starts on when nothing was chosen before. */
export const FIRST_PAGE: PageId = 'setup';

/** Pages that moved: the old address still lands somewhere sensible. */
const MOVED: Record<string, PageId> = { safe: 'general', connections: 'setup', recording: 'video', updates: 'about', infotypes: 'layouts' };

/** Resolve a page name from an address, a saved choice or the backend, following moves. */
export function resolvePage(v: string | null | undefined): PageId | null {
  if (!v) return null;
  if (isPageId(v)) return v;
  return MOVED[v] ?? null;
}

export function isPageId(v: string | null | undefined): v is PageId {
  return PAGES.some((p) => p.id === v);
}
