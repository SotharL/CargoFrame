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

export type ItemKind = 'trigger' | 'layout' | 'rule' | 'infotype' | 'connection';

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

// ----- connections and sharing (mirror core/connectors and app/src/share.rs) -----

export type ServiceKind = 'jira' | 'google' | 'reddit' | 'claude' | 'custom';
export type ConnectionState = 'connected' | 'disconnected' | 'expired' | 'disabled';

export interface Pair {
  name: string;
  value: string;
}

export type CustomAuth =
  | { type: 'none' }
  | { type: 'bearer' }
  | { type: 'header'; name: string }
  | { type: 'basic'; user: string }
  | { type: 'query'; param: string };

export type CustomBody =
  | { type: 'none' }
  | { type: 'json'; template: string }
  | { type: 'form'; fields: Pair[] }
  | { type: 'multipart'; fields: Pair[]; image_field: string; files_field: string };

export interface CustomList {
  id: string;
  label: string;
  path: string;
  items_path: string;
  value_path: string;
  label_path: string;
}

interface ConnectionCommon {
  name: string;
  enabled: boolean;
}

export type Connection = ConnectionCommon &
  (
    | { kind: 'jira'; site_url: string; auth: 'api_token' | 'pat'; email: string }
    | { kind: 'google'; client_id: string; sheets: boolean; full_drive: boolean }
    | { kind: 'reddit'; client_id: string; redirect_port: number }
    | { kind: 'claude'; open_with: 'auto' | 'app' | 'browser'; image_and_text_together: boolean }
    | {
        kind: 'custom';
        base_url: string;
        auth: CustomAuth;
        send: { method: 'POST' | 'PUT' | 'PATCH'; path: string; headers: Pair[]; body: CustomBody };
        lists: CustomList[];
        test_path: string;
        result_url_path: string;
      }
  );

export interface FieldSpec {
  key: string;
  label: string;
  hint: string;
  list: string | null;
}

export interface ListSpec {
  id: string;
  label: string;
  /** The list needs a value after a colon, for example a project key. */
  param: string | null;
}

export interface Describe {
  fields: FieldSpec[];
  lists: ListSpec[];
  summary: string;
}

export interface ConnectionView {
  id: string;
  name: string;
  kind: ServiceKind;
  kind_label: string;
  enabled: boolean;
  state: ConnectionState;
  /** Which of the secrets the person types are stored (never their values). */
  secrets: { name: string; stored: boolean }[];
  describe: Describe;
}

export interface ConnectionsView {
  items: ConnectionView[];
  problems: string[];
  /** Set when the operating system's credential store cannot be used. */
  store_problem: string | null;
}

export interface ChatView {
  id: string;
  title: string;
  pinned: boolean;
}

/** What a failed list request carries. */
export interface OptionsError {
  message: string;
  reconnect: boolean;
}

export interface SavedCopy {
  place: 'safe_folder' | 'picked_folder' | 'kept_locally' | 'not_saved';
  path: string;
  note: string | null;
}

export type ShareResult =
  | {
      status: 'sent';
      summary: string;
      url: string | null;
      notes: string[];
      copy_next: string | null;
      close: boolean;
    }
  | { status: 'blocked'; reasons: string[] }
  | {
      status: 'failed';
      label: string;
      message: string;
      reconnect: boolean;
      attempts: number;
      saved: SavedCopy | null;
    };

/** Everything a sharing button sends, apart from the picture itself. */
export interface ShareRequest {
  session_id: number;
  trigger_id: string;
  button_id: string;
  /** The button's destination as the layout names it (the backend reads it from the layout). */
  connection_id: string | null;
  /** The Prompt text block bound to the button, with what the person typed. */
  prompt: string | null;
  texts: Record<string, string>;
  dropdowns: Record<string, string | null>;
  files: Record<string, string[]>;
  /** True when the person changed the picture (the edited PNG is uploaded first). */
  image_edited: boolean;
}
