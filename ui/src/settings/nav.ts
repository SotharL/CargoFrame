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

export const GROUP_TITLES: Record<PageGroup, string> = {
  start: 'Get started',
  capture: 'Captures',
  app: 'CargoFrame',
};

/** Order matches the settings window's sidebar: the order a new person sets things up in. */
export const PAGES: PageMeta[] = [
  { id: 'setup', group: 'start', title: 'Setup', blurb: 'What CargoFrame uses on this computer, then the services you share to.' },
  { id: 'layouts', group: 'capture', title: 'Layouts', blurb: 'Build the window each trigger opens, block by block.' },
  { id: 'triggers', group: 'capture', title: 'Triggers', blurb: 'Shortcuts that capture your screen and open a layout.' },
  { id: 'rules', group: 'capture', title: 'Data & fields', blurb: 'Read values from files in the project folder (a machine config, a version), then fill the service’s fields with them (Jira labels, priority…).' },
  { id: 'video', group: 'capture', title: 'Recording', blurb: 'How videos are recorded on this PC: where ffmpeg is, how it captures and compresses, and a short test.' },
  { id: 'general', group: 'app', title: 'General', blurb: 'How CargoFrame looks, starts, tells you things, keeps windows ready, and what it does when a share fails.' },
  { id: 'data', group: 'app', title: 'Your data', blurb: 'What CargoFrame keeps on this computer, a backup of your layouts, and a way to start again.' },
  { id: 'about', group: 'app', title: 'About', blurb: 'The version, updates, and how fast CargoFrame has been on this computer.' },
];

/** The page a new window starts on when nothing was chosen before. */
export const FIRST_PAGE: PageId = 'setup';

/** Pages that moved: the old address still lands somewhere sensible. */
const MOVED: Record<string, PageId> = { safe: 'general', connections: 'setup', recording: 'video', updates: 'about', infotypes: 'rules' };

/** Resolve a page name from an address, a saved choice or the backend, following moves. */
export function resolvePage(v: string | null | undefined): PageId | null {
  if (!v) return null;
  if (isPageId(v)) return v;
  return MOVED[v] ?? null;
}

export function isPageId(v: string | null | undefined): v is PageId {
  return PAGES.some((p) => p.id === v);
}
