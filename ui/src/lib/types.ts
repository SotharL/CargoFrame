export type ThemeChoice = 'system' | 'light' | 'dark';
export type MemoryMode = 'balanced' | 'low_memory' | 'always_warm';

/** Mirrors `general.json` (without the schema version, which only the backend handles). */
export interface General {
  theme: ThemeChoice;
  memory_mode: MemoryMode;
  check_updates: boolean;
}

export interface AppInfo {
  version: string;
  os: 'windows' | 'linux' | 'macos' | 'other';
  config_dir: string;
  data_dir: string;
  /** False when a Linux desktop has no tray host (for example GNOME without the extension). */
  tray_available: boolean;
}

// ----- layouts (mirror core/layout) -----

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface InfoMapping {
  field: string;
  source: { kind: 'block' | 'rule' | 'fixed'; value: string };
}

export type DropdownSource =
  | { type: 'web_api'; connection_id: string; options_id: string }
  | { type: 'claude_chats' };

export type BlockContent =
  | { type: 'screenshot' }
  | {
      type: 'text';
      title: string;
      placement: 'above' | 'inside';
      role: 'plain' | 'prompt';
      default_text: string;
    }
  | {
      type: 'sharing_button';
      label: string;
      connection_id: string | null;
      prompt_block_id: string | null;
      mappings: InfoMapping[];
      on_success: 'close_window' | 'keep_open';
    }
  | { type: 'dropdown'; title: string; source: DropdownSource }
  | { type: 'files'; title: string; include: string[]; preselect: string[] };

export type Block = { id: string; rect: Rect } & BlockContent;

export interface Layout {
  id: string;
  name: string;
  blocks: Block[];
}

// ----- the CargoFrame window's data (mirror core/window::view) -----

export type Severity = 'error' | 'warning' | 'debug';

export interface IssueView {
  block_id: string | null;
  severity: Severity;
  code: string;
  message: string;
}

export interface FileEntry {
  path: string;
  size: number;
  selected: boolean;
}

export interface FilesState {
  entries: FileEntry[];
  truncated: boolean;
  problem: string | null;
}

export interface DropdownOption {
  value: string;
  label: string;
  group: string | null;
}

export interface DropdownState {
  status: 'ready' | 'loading' | 'needs_connection';
  options: DropdownOption[];
  selected: string | null;
  message: string | null;
}

export interface SessionView {
  session_id: number;
  trigger_id: string;
  trigger_name: string;
  layout: Layout | null;
  layout_problem: string | null;
  subject: 'window' | 'display' | 'all_displays';
  notes: string[];
  app: { title: string; exe_path: string | null; exe_name: string | null } | null;
  project: { folder: string; name: string; how: string } | null;
  image: { width: number; height: number };
  variables: Record<string, string>;
  texts: Record<string, { value: string; unresolved: string[] }>;
  files: Record<string, FilesState>;
  dropdowns: Record<string, DropdownState>;
  issues: IssueView[];
  sharing: Record<string, { enabled: boolean; reasons: string[] }>;
}
