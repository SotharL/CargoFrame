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

// ----- configuration items edited in Settings (mirror core/config, core/rules) -----

export type ItemKind = 'trigger' | 'layout' | 'rule' | 'infotype';

export interface ItemEntry<T> {
  id: string;
  item: T;
}

export interface ItemList<T> {
  items: ItemEntry<T>[];
  /** Files that could not be read; left untouched on disk. */
  problems: string[];
}

export type CaptureMode = 'active_window' | 'display' | 'all_displays';

export interface Trigger {
  name: string;
  enabled: boolean;
  /** Canonical, e.g. `Ctrl+Shift+F9`. Empty means "tray menu only". */
  shortcut: string;
  capture: CaptureMode;
  layout_id: string | null;
}

export type RuleFormat = 'json' | 'yaml' | 'toml' | 'ini' | 'csv' | 'xml' | 'regex';
export type RuleMultiple = 'first' | 'last' | 'join';

export interface Rule {
  name: string;
  file: string;
  format: RuleFormat;
  query: string;
  multiple: RuleMultiple;
  separator: string;
  fallback: string | null;
  max_bytes: number;
}

export interface RuleTest {
  folder: string | null;
  value: string | null;
  problem: string | null;
}

export type InfoKind = 'text' | 'choice' | 'multi_choice' | 'number' | 'flag';

export type InfoOptions =
  | { type: 'none' }
  | { type: 'fixed'; options: { value: string; label: string }[] }
  | { type: 'api'; options_id: string };

export interface InfoType {
  name: string;
  connection_id: string;
  field: string;
  kind: InfoKind;
  options: InfoOptions;
  required: boolean;
  default_template: string;
}

export interface Safe {
  /** Folder for the fallback zip; null means "ask me". */
  folder: string | null;
  timeout_secs: number;
  max_retries: number;
}

export interface VariableInfo {
  name: string;
  description: string;
}
