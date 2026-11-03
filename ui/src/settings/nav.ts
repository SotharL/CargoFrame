export type PageId =
  | 'general'
  | 'connections'
  | 'triggers'
  | 'rules'
  | 'infotypes'
  | 'layouts';

export interface PageMeta {
  id: PageId;
  title: string;
  blurb: string;
}

/** Order matches the settings window's sidebar. */
export const PAGES: PageMeta[] = [
  { id: 'general', title: 'General', blurb: 'Appearance, startup, safety, export and import, files, performance and updates.' },
  { id: 'connections', title: 'Connections', blurb: 'Services you share to: Jira, Google, Reddit, Claude, Slack, Discord, or your own API.' },
  { id: 'triggers', title: 'Triggers', blurb: 'Shortcuts that capture your screen and open a CargoFrame window.' },
  { id: 'rules', title: 'Data from files', blurb: 'Pull a value out of a file in the project folder (a version, a log line) to use in your text and fields.' },
  { id: 'infotypes', title: 'Destination fields', blurb: 'The boxes on the service you share to (Jira priority, labels…) and what fills each one.' },
  { id: 'layouts', title: 'Layouts', blurb: 'Build the window each trigger opens, block by block.' },
];

/** Pages that moved: the old address still lands somewhere sensible. */
const MOVED: Record<string, PageId> = { safe: 'general' };

/** Resolve a page name from an address, a saved choice or the backend, following moves. */
export function resolvePage(v: string | null | undefined): PageId | null {
  if (!v) return null;
  if (isPageId(v)) return v;
  return MOVED[v] ?? null;
}

export function isPageId(v: string | null | undefined): v is PageId {
  return PAGES.some((p) => p.id === v);
}
