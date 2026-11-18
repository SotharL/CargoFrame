import { msg, translated } from '../lib/i18n/index.svelte';
import type { IconName } from '../lib/icons';

export type PageId =
  | 'connections'
  | 'layouts'
  | 'triggers'
  | 'rules'
  | 'video'
  | 'preferences'
  | 'files'
  | 'system';

/** The sidebar is in three groups: where captures go, what a capture does, and the program itself. */
export type PageGroup = 'start' | 'capture' | 'app';

export interface PageMeta {
  id: PageId;
  title: string;
  blurb: string;
  group: PageGroup;
  /** Its picture in the sidebar (lib/icons.ts): beside the title, or alone when the sidebar is folded. */
  icon: IconName;
}

export const GROUP_TITLES: Record<PageGroup, string> = translated({
  start: msg('Get started'),
  capture: msg('Captures'),
  app: msg('CargoFrame'),
}, 'all');

/** Order matches the settings window's sidebar: the order a new person sets things up in. */
export const PAGES: PageMeta[] = translated([
  { id: 'connections', group: 'start', icon: 'connections', title: msg('Connections'), blurb: msg('The services you share to: Jira, GitHub, Slack, Discord, Claude and more, or your own API.') },
  { id: 'layouts', group: 'capture', icon: 'layouts', title: msg('Layouts'), blurb: msg('The window each shortcut opens, block by block, and what fills the fields of a service.') },
  { id: 'triggers', group: 'capture', icon: 'shortcuts', title: msg('Shortcuts'), blurb: msg('Key combinations that capture your screen, or record it, and open a layout.') },
  { id: 'rules', group: 'capture', icon: 'parsing', title: msg('Data parsing'), blurb: msg('Values read from the files of a capture, like the map in a log or a version, for layouts to write in titles, tags and fields.') },
  { id: 'video', group: 'capture', icon: 'recording', title: msg('Recording'), blurb: msg('How videos are recorded: what shows on screen, how long a clip is, how ffmpeg captures and compresses.') },
  { id: 'preferences', group: 'app', icon: 'preferences', title: msg('Preferences'), blurb: msg('Language, theme and colour, notifications, and starting with the computer.') },
  { id: 'files', group: 'app', icon: 'files', title: msg('Files & backup'), blurb: msg('Where failed shares are saved, what CargoFrame keeps on this computer, export, import and reset.') },
  { id: 'system', group: 'app', icon: 'system', title: msg('System'), blurb: msg('The apps CargoFrame uses on this computer, updates, memory and speed.') },
]);

/** The page a new window starts on when nothing was chosen before. */
export const FIRST_PAGE: PageId = 'connections';

/** Pages that moved: the old address still lands somewhere sensible. */
const MOVED: Record<string, PageId> = {
  setup: 'system',
  general: 'preferences',
  data: 'files',
  about: 'system',
  safe: 'files',
  updates: 'system',
  recording: 'video',
  infotypes: 'layouts',
};

/** Resolve a page name from an address, a saved choice or the backend, following moves. */
export function resolvePage(v: string | null | undefined): PageId | null {
  if (!v) return null;
  if (isPageId(v)) return v;
  return MOVED[v] ?? null;
}

export function isPageId(v: string | null | undefined): v is PageId {
  return PAGES.some((p) => p.id === v);
}
