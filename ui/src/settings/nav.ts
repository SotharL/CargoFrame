export type PageId =
  | 'triggers'
  | 'connections'
  | 'rules'
  | 'infotypes'
  | 'layouts'
  | 'safe'
  | 'general';

export interface PageMeta {
  id: PageId;
  title: string;
  blurb: string;
}

/** Order matches the settings window's sidebar. */
export const PAGES: PageMeta[] = [
  { id: 'triggers', title: 'Triggers', blurb: 'Shortcuts that capture your screen and open a CargoFrame window.' },
  { id: 'connections', title: 'Connections', blurb: 'Services you share to: Jira, Google, Reddit, Claude, or your own API.' },
  { id: 'rules', title: 'Data rules', blurb: 'Values CargoFrame reads from files in the project folder.' },
  { id: 'infotypes', title: 'Info types', blurb: 'The named fields a connection accepts, and where each value comes from.' },
  { id: 'layouts', title: 'Layouts', blurb: 'Build the window each trigger opens, block by block.' },
  { id: 'safe', title: 'Safe settings', blurb: 'What happens when sharing fails.' },
  { id: 'general', title: 'General', blurb: 'Appearance, startup and memory.' },
];

export function isPageId(v: string | null | undefined): v is PageId {
  return PAGES.some((p) => p.id === v);
}
