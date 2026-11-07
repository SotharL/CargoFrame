export type ThemeChoice = 'system' | 'light' | 'dark';
export type MemoryMode = 'balanced' | 'low_memory' | 'always_warm';

/** Mirrors `general.json` (without the schema version, which only the backend handles). */
export interface General {
  theme: ThemeChoice;
  memory_mode: MemoryMode;
  check_updates: boolean;
  /** `#rrggbb`: buttons, focus rings, links and the arrow of the logo. */
  accent: string;
}

/** One line of the last timings (Settings, General). */
export interface TimingView {
  id: string;
  label: string;
  ms: number;
  budget_ms: number | null;
  /** 'cold' or 'warm' for windows. */
  note: string | null;
}

/** One entry of a searchable drop down (`Combobox`). */
export interface ComboOption {
  value: string;
  label: string;
  /** Shown small, after the label: a group name or the field key. */
  note?: string | null;
  /** A heading the entry is listed under; the list shows it where the heading changes. */
  group?: string | null;
}

/** What the memory setting is doing to one window right now (Settings, General, Performance). */
export interface WindowStatus {
  label: string;
  name: string;
  /** 'open' on screen, 'kept' hidden but alive, 'closed' not there. */
  state: 'open' | 'kept' | 'closed';
}

export interface MemoryStatus {
  mode: MemoryMode;
  windows: WindowStatus[];
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

/** One app CargoFrame can use, and whether it is on this computer (mirrors app/src/setup.rs). */
export interface SetupItem {
  id: string;
  name: string;
  /** What does not work without it. */
  what_for: string;
  status: 'ok' | 'missing' | 'unknown';
  /** Where it was found, or what is wrong. */
  detail: string;
  /** A button on this line: open the Windows sound settings. */
  action: 'sound_settings' | null;
}

/** What the one button on the Setup page does. */
export type SetupInstall =
  | { type: 'none' }
  | { type: 'run'; label: string }
  | { type: 'copy'; command: string }
  | { type: 'manual'; text: string }
  | { type: 'running' }
  | { type: 'failed'; message: string };

export interface SetupReport {
  os: 'windows' | 'linux' | 'other';
  items: SetupItem[];
  /** How many are missing: the number on the Setup tab. */
  missing: number;
  install: SetupInstall;
}

/** What a video block records: the whole screen, or the window that has the focus when it starts. */
export type VideoSubject = 'screen' | 'focused_window';

/** Where one sound comes from: off, the system's own choice, or one device by name. */
export type AudioChoice = 'off' | 'default' | { device: string };

/** The sounds in a recording: what the computer plays, and what a microphone hears. */
export interface VideoAudio {
  desktop: AudioChoice;
  microphone: AudioChoice;
}

/** What a new video block records: the screen, with the computer's sound and no microphone. */
export const DEFAULT_VIDEO_AUDIO: VideoAudio = { desktop: 'default', microphone: 'off' };

/** A sound device the person can pick (mirrors app/src/audio.rs). */
export interface AudioDevice {
  /** What is stored in the layout and given to ffmpeg. */
  name: string;
  label: string;
  default: boolean;
}

/** The sound devices of this computer, by what they carry. */
export interface AudioDevices {
  os: 'windows' | 'linux' | 'other';
  /** Carry what the computer plays. */
  desktop: AudioDevice[];
  /** Carry what a microphone hears. */
  microphone: AudioDevice[];
  /** ffmpeg answered. */
  listed: boolean;
  /** What the person should know about their choices. */
  note: string | null;
}

export type BlockContent =
  | { type: 'screenshot' }
  | { type: 'video'; subject?: VideoSubject; audio?: VideoAudio }
  | {
      type: 'text';
      title: string;
      placement: 'above' | 'inside';
      role: 'plain' | 'prompt' | 'zip_name';
      default_text: string;
    }
  | {
      type: 'sharing_button';
      label: string;
      connection_id: string | null;
      prompt_block_id: string | null;
      mappings: InfoMapping[];
      on_success: 'close_window' | 'keep_open';
      /** What goes with the text. Absent while it is the default (picture, files attached). */
      send?: SendRules;
    }
  | {
      type: 'dropdown';
      title: string;
      source: DropdownSource;
      /** Keep the last five choices at the top and start on the last used. Absent: on. */
      remember?: boolean;
      /** May stay empty, or hold something typed or pasted that is not in the list. */
      optional?: boolean;
      /** A text block whose words narrow the list while it is shown. */
      narrow_by?: string | null;
    }
  | { type: 'files'; title: string; include: string[]; preselect: string[]; look_in: LookIn };

/** What happens to the ticked files: sent as files, written into the message, or left out. */
export type FilesHow = 'attach' | 'inline' | 'none';

export interface SendRules {
  screenshot: boolean;
  files: FilesHow;
  /** With `files` set to `inline`: the most text, in kilobytes, written into the message. */
  inline_limit_kb: number;
}

export const DEFAULT_SEND_RULES: SendRules = { screenshot: true, files: 'attach', inline_limit_kb: 64 };
export const MAX_INLINE_KB = 512;

/** A folder a Files block can look in, found from the program that had focus. */
export type LookFolder = 'project' | 'program' | 'above_1' | 'above_2' | 'above_3';

export interface LookIn {
  /** Empty means the project folder. */
  folders: LookFolder[];
  /** A folder inside each of the folders above, such as logs; empty for the folder itself. */
  subfolder: string;
}

/** One folder a Files block looked in, and how many files it offered. */
export interface PlaceFound {
  label: string;
  folder: string;
  exists: boolean;
  found: number;
}

/** What a Files block would find, for the builder's live count. */
export interface FilesPreview {
  program: string | null;
  places: PlaceFound[];
  missing: string[];
  total: number;
  ticked: number;
  truncated: boolean;
  problem: string | null;
}

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
  /** What each folder looked in held; the window says where files came from when there are several. */
  places: PlaceFound[];
}

export interface DropdownOption {
  value: string;
  label: string;
  group: string | null;
  /** Where the choice lives, for a list the window narrows step by step: the server... */
  server?: string | null;
  /** ...and, inside it, the category (or the channel a thread is in). */
  category?: string | null;
}

export interface DropdownState {
  status: 'ready' | 'loading' | 'needs_connection';
  options: DropdownOption[];
  selected: string | null;
  message: string | null;
}

/** What the window knows of the recording of a video capture. */
export interface VideoClipInfo {
  width: number;
  height: number;
  duration_ms: number;
  bytes: number;
  /** The recording has a sound track; without one the player shows no volume. */
  audio: boolean;
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
  /** The recording, for a capture made by a video trigger; absent for a screenshot. */
  video?: VideoClipInfo | null;
  variables: Record<string, string>;
  texts: Record<string, { value: string; unresolved: string[] }>;
  files: Record<string, FilesState>;
  dropdowns: Record<string, DropdownState>;
  issues: IssueView[];
  sharing: Record<string, SharingView>;
}

// ----- configuration items edited in Settings (mirror core/config, core/rules) -----

export type ItemKind = 'trigger' | 'layout' | 'rule' | 'infotype' | 'connection';

/** A ready-made layout to start from. `service` is the kind of connection it is built for. */
export interface LayoutTemplate {
  key: string;
  name: string;
  description: string;
  /** `null`: the one that saves locally and needs no connection. */
  service: 'jira' | 'google' | 'reddit' | 'claude' | 'slack' | 'discord' | 'teams' | 'custom' | null;
}

// ----- export and import of settings -----

/** The groups a settings file holds. Connections, shortcuts and secrets are never among them. */
export type TransferGroup = 'layouts' | 'rules' | 'infotypes';

export interface ExportSelection {
  group: TransferGroup;
  ids: string[];
}

export interface ImportPreviewItem {
  group: TransferGroup;
  id: string;
  name: string;
  /** An item with this id is already here. */
  exists: boolean;
  existing_name: string | null;
  /** Why it cannot be imported, when it cannot. */
  error: string | null;
  notes: string[];
  /** Whether it can be brought in next to the one that is here (not for data rules). */
  can_copy: boolean;
}

export interface ImportPreview {
  file_name: string;
  items: ImportPreviewItem[];
}

export type ImportAction = 'skip' | 'replace' | 'copy';

export interface ImportPick {
  group: TransferGroup;
  id: string;
  action: ImportAction;
}

export interface ImportReport {
  imported: { group: TransferGroup; id: string; name: string; copy: boolean }[];
  skipped: number;
  failed: string[];
}

export interface ItemEntry<T> {
  id: string;
  item: T;
}

export interface ItemList<T> {
  items: ItemEntry<T>[];
  /** Files that could not be read; left untouched on disk. */
  problems: string[];
}

/** A screenshot, or a video (the first press starts recording, the next one stops). */
export type Media = 'screenshot' | 'video';

export type CaptureMode = 'active_window' | 'display' | 'all_displays';

export interface Trigger {
  name: string;
  enabled: boolean;
  /** Canonical, e.g. `Ctrl+Shift+F9`. Empty means "tray menu only". */
  shortcut: string;
  capture: CaptureMode;
  /** What the shortcut makes: a picture of the screen, or a recording of it. */
  media: Media;
  layout_id: string | null;
}

export type RuleFormat = 'json' | 'yaml' | 'toml' | 'ini' | 'csv' | 'xml' | 'regex' | 'text' | 'file';
export type RuleMultiple = 'first' | 'last' | 'join' | 'nth';

export interface Rule {
  name: string;
  file: string;
  format: RuleFormat;
  query: string;
  multiple: RuleMultiple;
  separator: string;
  /** Which one when `multiple` is `nth`: 1 is the first, -1 the last. */
  nth: number;
  /** Regex only: the value built from each match, such as `$1.$2`. Empty gives group 1 or the match. */
  output: string;
  fallback: string | null;
  max_bytes: number;
}

export interface RuleTest {
  folder: string | null;
  value: string | null;
  problem: string | null;
  /** The file the rule's pattern means, relative to the folder, and the start of it. */
  file: string | null;
  excerpt: string | null;
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

// ----- video (mirror core/config Video and app/src/video.rs) -----

export type VideoCapture = 'auto' | 'gdigrab' | 'ddagrab' | 'x11';
export type VideoEncoder = 'auto' | 'software' | 'nvenc' | 'qsv' | 'amf' | 'media_foundation' | 'vaapi';

export interface Video {
  /** Folder that holds ffmpeg and ffprobe, or the ffmpeg program; null means "find it". */
  ffmpeg: string | null;
  capture: VideoCapture;
  encoder: VideoEncoder;
  fps: number;
  /** Seconds before the stop shortcut the clip may start. */
  window_secs: number;
  /** Larger pictures are scaled down to this height; 0 keeps the screen's size. */
  max_height: number;
}

export interface VideoTools {
  found: boolean;
  ffmpeg: string | null;
  version: string | null;
  /** Encoders this ffmpeg lists. Listed does not mean this PC can use it. */
  encoders: VideoEncoder[];
  captures: VideoCapture[];
  auto_capture: VideoCapture | null;
  auto_encoder: VideoEncoder | null;
  os: 'windows' | 'linux' | 'other';
  problem: string | null;
}

/** What "Export a copy" does with the test clip (mirrors app/src/video.rs `TestEdit`). */
export interface VideoTestEdit {
  start_ms: number;
  end_ms: number | null;
  crop: { x: number; y: number; w: number; h: number } | null;
  /** A mark laid over the whole copy: none, a square ffmpeg draws, or a picture the page draws. */
  mark: 'none' | 'square' | 'drawn';
}

/** A recording started with "Record until I stop". */
export type VideoRecording =
  | { state: 'idle' }
  | { state: 'recording'; elapsed_ms: number; buffered_ms: number; squeezed: boolean; keep_secs: number }
  | { state: 'finishing' }
  | { state: 'failed'; message: string };

export interface VideoTrial {
  ok: boolean;
  message: string;
  capture: VideoCapture | null;
  encoder: VideoEncoder | null;
  width: number;
  height: number;
  clip_ms: number;
  clip_bytes: number;
  took_ms: number;
  clip: string | null;
}

export interface VariableInfo {
  name: string;
  description: string;
}

// ----- connections and sharing (mirror core/connectors and app/src/share.rs) -----

export type ServiceKind = 'jira' | 'google' | 'reddit' | 'claude' | 'slack' | 'discord' | 'teams' | 'custom';
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
    | { kind: 'claude'; open_with: 'auto' | 'app' | 'browser'; send_automatically: boolean; extra_wait_seconds: number }
    | { kind: 'slack' }
    | {
        kind: 'discord';
        /** `bot` is what connections saved before webhooks were. */
        mode: 'webhook' | 'bot';
        shown_as: string;
        hooks: { slot: number; name: string; server: string }[];
      }
    | { kind: 'teams'; client_id: string; tenant: string }
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

export type Support = 'yes' | 'limited' | 'no';

export interface Capability {
  support: Support;
  /** One line: how it is used, or why it cannot be. */
  note: string;
}

/** What a destination can take: the one table behind the builder, the window and the share. */
export interface Capabilities {
  text: Capability;
  dropdown: Capability;
  screenshot: Capability;
  video: Capability;
  files: Capability;
  link: Capability;
}

/** How a block takes part in a share. */
export type Role =
  | { role: 'screenshot' }
  | { role: 'video' }
  | { role: 'files' }
  | { role: 'prompt' }
  | { role: 'zip_name' }
  | { role: 'field'; field: string };

export type Verdict =
  | ({ state: 'sent' } & Role)
  | { state: 'cannot_send'; why: string }
  | { state: 'switched_off' }
  | { state: 'not_used' };

export type BlockReach = {
  block_id: string;
  /** The button cannot do its job without this block: its box stays ticked. */
  required?: boolean;
} & Verdict;

/** What one sharing button does with every other block of the layout. */
export interface Reach {
  capabilities: Capabilities;
  blocks: BlockReach[];
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
  /** What a button to this connection can send. */
  capabilities: Capabilities;
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

/** What a sharing button does and whether it can right now. */
export interface SharingView {
  enabled: boolean;
  reasons: string[];
  /** `save_locally` when the button has no active connection. */
  mode: 'send' | 'save_locally';
  /** What pressing it does, when that is not obvious. */
  note: string | null;
  /** The kind of destination (`jira`, `claude`...), which gives the button its colour. Null when it saves a zip here. */
  kind: ServiceKind | null;
  /** What this button does with each other block, and what its destination can take. */
  reach: Reach | null;
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
      close: boolean;
    }
  | { status: 'saved_locally'; path: string; notes: string[]; close: boolean }
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
  /** For a recording: what was cut or cropped. Left out (or `null`) sends it as it is. */
  clip_edit?: ClipEdit | null;
  /**
   * The blocks whose boxes are ticked for this button. Left out (or `null`) sends what the layout
   * says; a list sends exactly those blocks.
   */
  send_blocks?: string[] | null;
  /** The window found nowhere to post (an empty list): keep a zip here instead of sending. */
  save_here?: boolean;
}

/**
 * What the person did to a recording: kept from `start_ms` to `end_ms` (`null` is the end of the
 * recording), and perhaps cropped to an area, in the recording's own pixels.
 */
export interface ClipEdit {
  start_ms: number;
  end_ms: number | null;
  crop?: { x: number; y: number; w: number; h: number } | null;
  /** A mark was drawn: the window uploads it just before the share. */
  mark?: boolean;
}

// ----- updates (Settings, General) -----

/** Where an update is. */
export type UpdatePhase =
  | { state: 'idle' }
  | { state: 'checking' }
  | { state: 'downloading'; done: number; total: number | null }
  | { state: 'verifying' }
  | { state: 'installing' }
  | {
      state: 'failed';
      message: string;
      /** The verified installer, kept so it can be run by hand. */
      installer: string | null;
      needs_token: boolean;
    };

/** The outcome of the last check. */
export type UpdateLast =
  | { state: 'up_to_date'; latest: string }
  | {
      state: 'available';
      version: string;
      notes: string;
      page: string;
      installer: string;
      size: number;
      published: string | null;
    }
  | { state: 'failed'; message: string; needs_token: boolean };

export interface UpdateView {
  current: string;
  repository: string;
  /** There is an installer for this kind of computer. */
  supported: boolean;
  /** A GitHub token is stored. The token itself is never sent to the window. */
  token_set: boolean;
  /** The password store could not be read, so a token cannot be kept or used. */
  token_problem: string | null;
  last: UpdateLast | null;
  phase: UpdatePhase;
}
