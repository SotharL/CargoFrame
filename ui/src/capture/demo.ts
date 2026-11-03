import type { SessionView, ShareResult } from '../lib/types';

/**
 * A sample capture for `npm run dev`, tests and screenshots. Loaded only outside the app, so it
 * is not part of the real window's code.
 */
export async function demoSession(): Promise<SessionView> {
  // `?mode=local` shows the window as it is when the button has no active connection.
  const local = new URLSearchParams(location.search).get('mode') === 'local';
  return {
    session_id: 1,
    trigger_id: 'bug',
    trigger_name: 'Bug report',
    subject: 'window',
    notes: [],
    app: { title: 'Foo - main.rs', exe_path: '/home/me/dev/Foo/target/debug/Foo', exe_name: 'Foo' },
    project: { folder: '/home/me/dev/Foo', name: 'Foo', how: 'the folder above has the program\'s name' },
    image: { width: 960, height: 600 },
    variables: { 'project.name': 'Foo', 'app.exe': 'Foo', 'capture.date': '2026-10-23' },
    layout_problem: null,
    layout: {
      id: 'bug',
      name: 'Bug report',
      blocks: [
        { id: 'shot', type: 'screenshot', rect: { x: 0, y: 0, w: 8, h: 9 } },
        {
          id: 'prompt', type: 'text', rect: { x: 8, y: 0, w: 4, h: 3 },
          title: 'Prompt', placement: 'above', role: 'prompt', default_text: 'What is wrong in {{project.name}}?',
        },
        {
          id: 'note', type: 'text', rect: { x: 8, y: 3, w: 4, h: 2 },
          title: 'Note', placement: 'inside', role: 'plain', default_text: 'Seen on {{capture.date}} {{rule.build}}',
        },
        { id: 'chat', type: 'dropdown', rect: { x: 8, y: 5, w: 4, h: 2 }, title: 'Claude chat', source: { type: 'claude_chats' } },
        {
          id: 'proj', type: 'dropdown', rect: { x: 8, y: 7, w: 4, h: 2 }, title: 'Jira project',
          source: { type: 'web_api', connection_id: 'jira', options_id: 'projects' },
        },
        { id: 'logs', type: 'files', rect: { x: 0, y: 9, w: 8, h: 3 }, title: 'Logs', include: ['**/*.log'], preselect: ['*.log'], look_in: { folders: ['project'], subfolder: '' } },
        {
          id: 'send', type: 'sharing_button', rect: { x: 8, y: 9, w: 4, h: 2 },
          label: 'Share', connection_id: 'jira', prompt_block_id: null, mappings: [], on_success: 'keep_open',
        },
      ],
    },
    texts: {
      prompt: { value: 'What is wrong in Foo?', unresolved: [] },
      note: { value: 'Seen on 2026-10-23 {{rule.build}}', unresolved: ['rule.build'] },
    },
    files: {
      logs: {
        truncated: false,
        problem: null,
        places: [],
        entries: [
          { path: 'app.log', size: 18_204, selected: true },
          { path: 'logs/debug.log', size: 2_400_000, selected: false },
          { path: 'logs/old/2026-10-01.log', size: 912, selected: false },
        ],
      },
    },
    dropdowns: {
      chat: {
        status: 'ready',
        selected: '01a168ea-ad83-718c-a341-75a44c6984ec',
        message: null,
        options: [
          { value: '01a168ea-ad83-718c-a341-75a44c6984ec', label: 'Release notes', group: 'Pinned' },
          { value: '11a168ea-ad83-718c-a341-75a44c6984ec', label: 'Bug triage', group: 'Recent' },
          { value: 'new', label: 'New chat', group: null },
        ],
      },
      proj: { status: 'loading', selected: null, options: [], message: null },
    },
    issues: [],
    sharing: local
      ? { send: { enabled: true, reasons: [], mode: 'save_locally', note: 'No connection: saves a zip in your “Folder for saved zips”' } }
      : { send: { enabled: true, reasons: [], mode: 'send', note: null } },
  };
}

/** A fake application window as the "screenshot". */
export async function demoImage(): Promise<Blob> {
  const w = 960;
  const h = 600;
  const c = document.createElement('canvas');
  c.width = w;
  c.height = h;
  const g = c.getContext('2d')!;
  g.fillStyle = '#1e2330';
  g.fillRect(0, 0, w, h);
  g.fillStyle = '#2b3245';
  g.fillRect(0, 0, w, 34);
  g.fillStyle = '#e04f4f';
  g.beginPath(); g.arc(18, 17, 6, 0, 7); g.fill();
  g.fillStyle = '#e0934f';
  g.beginPath(); g.arc(38, 17, 6, 0, 7); g.fill();
  g.fillStyle = '#62e04f';
  g.beginPath(); g.arc(58, 17, 6, 0, 7); g.fill();
  g.fillStyle = '#cfd6e6';
  g.font = '14px system-ui, sans-serif';
  g.fillText('Foo - main.rs', 90, 22);
  g.fillStyle = '#252c3d';
  g.fillRect(0, 34, 190, h - 34);
  const lines = ['fn main() {', '    let config = load();', '    let app = App::new(config);', '    app.run().expect("crashed");', '}'];
  g.font = '15px ui-monospace, monospace';
  lines.forEach((l, i) => {
    g.fillStyle = i === 3 ? '#e04f4f' : '#9fb4e0';
    g.fillText(l, 215, 80 + i * 26);
  });
  g.fillStyle = '#3a4560';
  for (let i = 0; i < 9; i++) g.fillRect(16, 60 + i * 30, 120 + ((i * 37) % 40), 10);
  g.fillStyle = '#2b3245';
  g.fillRect(190, 420, w - 190, 180);
  g.fillStyle = '#e04f4f';
  g.font = '15px ui-monospace, monospace';
  g.fillText('thread \'main\' panicked at src/main.rs:4: crashed', 215, 460);
  return new Promise((resolve) => c.toBlob((b) => resolve(b!), 'image/png'));
}

/** What a share answers in the browser demo, chosen with `?share=` in the address. */
export function demoShare(kind: string | null): ShareResult {
  switch (kind) {
    case 'fail':
      return {
        status: 'failed', label: 'Service error', message: 'Jira answered 503', reconnect: false, attempts: 4,
        saved: { place: 'safe_folder', path: '/home/me/Documents/CargoFrame zips/CargoFrame_2026-10-23_153045_jira.zip', note: null },
      };
    case 'expired':
      return {
        status: 'failed', label: 'Sign-in expired', message: 'Jira refused the token', reconnect: true, attempts: 1,
        saved: { place: 'kept_locally', path: '/home/me/.local/share/CargoFrame/kept/CargoFrame_x.zip', note: 'safe folder: folder not writable' },
      };
    case 'saved':
      return {
        status: 'saved_locally', path: '/home/me/Documents/CargoFrame zips/CargoFrame_2026-10-23_153045_local.zip',
        notes: [], close: false,
      };
    case 'blocked':
      return { status: 'blocked', reasons: ['\u201cSummary\u201d needs a value', 'Write the prompt before sharing'] };
    case 'handoff':
      return {
        status: 'sent', summary: 'Sent 2 files and the message to Claude', url: null,
        notes: ['CargoFrame cannot see inside Claude: check the chat that the message went through'], close: false,
      };
    case 'slow':
      return new Promise((resolve) => setTimeout(() => resolve({ status: 'blocked', reasons: ['done'] }), 1500)) as never;
    default:
      return {
        status: 'sent', summary: 'Created PROJ-123', url: 'https://example.atlassian.net/browse/PROJ-123',
        notes: ['1 file was skipped: logs/old.log could not be read'], close: false,
      };
  }
}
