import type { Layout, ServiceKind, SessionView, ShareResult } from '../lib/types';
import { demoCapabilities, demoReach, LOCAL_CAPABILITIES } from '../lib/demoReach';

/**
 * A sample capture for `npm run dev`, tests and screenshots. Loaded only outside the app, so it
 * is not part of the real window's code.
 */
export async function demoSession(): Promise<SessionView> {
  // `?mode=local` shows the window as it is when the button has no active connection.
  const params = new URLSearchParams(location.search);
  const local = params.get('mode') === 'local';
  // `?kind=claude`, `?kind=slack`, `?kind=discord`, `?kind=teams` or `?kind=reddit` points the button somewhere else, to look at its colours and marks.
  const asked = params.get('kind');
  const kind: ServiceKind = asked === 'claude' || asked === 'reddit' || asked === 'google' || asked === 'slack' || asked === 'discord' || asked === 'teams' || asked === 'custom' ? asked : 'jira';
  const layout: Layout = {
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
        label: 'Share', connection_id: 'jira', prompt_block_id: 'prompt', on_success: 'keep_open',
        mappings: [
          { field: 'description', source: { kind: 'block', value: 'note' } },
          { field: 'project', source: { kind: 'block', value: 'proj' } },
        ],
      },
    ],
  };
  // `?demo=discord` swaps the two dropdowns for one Discord path: server > category > channel,
  // then a thread inside it narrowed by the prompt, to look at the cascade.
  const discord = params.get('demo') === 'discord';
  if (discord) {
    layout.blocks = layout.blocks
      .filter((b) => b.id !== 'chat')
      .map((b) =>
        b.id === 'proj'
          ? ({
              id: 'proj', type: 'dropdown', rect: { x: 8, y: 5, w: 4, h: 4 }, title: 'Where to post', narrow_by: 'prompt',
              source: { type: 'web_api', connection_id: 'jira', options_id: 'channels' },
              deeper: { list: 'threads', field: 'thread' },
            } as typeof b)
          : b,
      );
  }
  // `?demo=tags` swaps the chat for a Tags block and adds a log viewer, to look at them.
  if (params.get('demo') === 'tags') {
    layout.blocks = layout.blocks.map((b) =>
      b.id === 'chat'
        ? ({ id: 'labels', type: 'tags', rect: { x: 8, y: 5, w: 4, h: 2 }, title: 'Labels', tags: ['crash', '{{rule.build}}'] } as typeof b)
        : b,
    );
    layout.blocks.push({ id: 'log', type: 'log_view', rect: { x: 0, y: 12, w: 12, h: 4 }, title: 'Log', filter: '' });
  }
  // `?buttons=3` adds buttons to Claude, Slack and Reddit (up to 5 with Google and a zip), to look at the boxes.
  const extra: [string, ServiceKind | null][] = [['claude', 'claude'], ['slack', 'slack'], ['reddit', 'reddit'], ['google', 'google']];
  const more = Math.max(1, Math.min(5, Number(params.get('buttons')) || 1)) - 1;
  const kinds = new Map<string, ServiceKind | null>();
  extra.slice(0, more).forEach(([id, k], i) => {
    layout.blocks.push({
      id: `send_${id}`, type: 'sharing_button', rect: { x: 8, y: 11 + i, w: 4, h: 1 },
      label: id === 'claude' ? 'Ask Claude' : `To ${id}`, connection_id: id, prompt_block_id: 'prompt', on_success: 'keep_open', mappings: [],
    });
    kinds.set(`send_${id}`, k);
  });
  // `?media=video` shows a recording where the picture would be; `?media=both` keeps the
  // screenshot block beside it (`&end=1` fills it with the picture taken when the recording stopped).
  const both = params.get('media') === 'both';
  const video = params.get('media') === 'video' || both;
  if (video && !both) layout.blocks[0] = { id: 'clip', type: 'video', rect: { x: 0, y: 0, w: 8, h: 9 } };
  if (both) {
    layout.blocks[0] = { id: 'shot', type: 'screenshot', rect: { x: 0, y: 0, w: 8, h: 4 } };
    layout.blocks.push({
      id: 'clip', type: 'video', rect: { x: 0, y: 4, w: 8, h: 5 }, screenshot_at_end: params.get('end') === '1',
    });
  }
  const button = layout.blocks.find((b) => b.type === 'sharing_button')!;
  const reach = demoReach(layout, button as Extract<typeof button, { type: 'sharing_button' }>, local ? LOCAL_CAPABILITIES : demoCapabilities(kind), local ? null : kind);
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
    layout,
    texts: {
      prompt: { value: 'What is wrong in Foo?', unresolved: [] },
      note: { value: 'Seen on 2026-10-23 {{rule.build}}', unresolved: ['rule.build'] },
    },
    tags: { labels: { tags: ['crash'], over: 0 } },
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
    dropdowns: discord
      ? {
          proj: { status: 'loading', selected: null, options: [], message: null },
        }
      : {
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
    ...(video ? { video: both ? { width: 640, height: 400, duration_ms: 1_300, bytes: 40_000, audio: false } : { width: 1280, height: 720, duration_ms: 9_500, bytes: 3 * 1024 * 1024, audio: true, ...demoExtras(params) } } : {}),
    issues: [],
    sharing: {
      ...(local
        ? { send: { enabled: true, reasons: [], mode: 'save_locally' as const, note: 'No connection: saves a zip in your “Folder for saved zips”', kind: null, reach } }
        : { send: { enabled: true, reasons: [], mode: 'send' as const, note: null, kind, reach } }),
      ...Object.fromEntries(
        [...kinds].map(([id, k]) => {
          const b = layout.blocks.find((x) => x.id === id) as Extract<(typeof layout.blocks)[number], { type: 'sharing_button' }>;
          return [id, { enabled: true, reasons: [], mode: 'send' as const, note: null, kind: k, reach: demoReach(layout, b, demoCapabilities(k!), k) }];
        }),
      ),
    },
  };
}

/** A short real recording (a moving bar over the fake window), so the player has a frame to show. */
export async function demoClip(): Promise<Blob> {
  const canvas = document.createElement('canvas');
  canvas.width = 640;
  canvas.height = 400;
  const g = canvas.getContext('2d')!;
  const stream = canvas.captureStream(15);
  const recorder = new MediaRecorder(stream, { mimeType: 'video/webm' });
  const parts: Blob[] = [];
  recorder.ondataavailable = (e) => parts.push(e.data);
  const done = new Promise<void>((resolve) => (recorder.onstop = () => resolve()));
  recorder.start();
  for (let i = 0; i < 20; i++) {
    g.fillStyle = '#1e2330';
    g.fillRect(0, 0, 640, 400);
    g.fillStyle = '#62e04f';
    g.fillRect(20 + i * 28, 160, 80, 80);
    g.fillStyle = '#cfd6e6';
    g.font = '20px system-ui, sans-serif';
    g.fillText(`frame ${i + 1}`, 24, 40);
    await new Promise((r) => setTimeout(r, 66));
  }
  recorder.stop();
  await done;
  return new Blob(parts, { type: 'video/mp4' });
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

/** `&keys=1` adds keys pressed to the demo recording, `&follow=1` a window that was moved. */
function demoExtras(params: URLSearchParams) {
  const keys = params.get('keys') === '1'
    ? [
        { from_ms: 500, to_ms: 1_200, text: 'Ctrl+S' },
        { from_ms: 2_000, to_ms: 2_200, text: 'h' },
        { from_ms: 2_200, to_ms: 2_400, text: 'hi' },
        { from_ms: 2_400, to_ms: 3_900, text: 'hi ⏎' },
      ]
    : undefined;
  const follow = params.get('follow') === '1'
    ? [
        { at_ms: 0, x: 100, y: 80 },
        { at_ms: 3_000, x: 100, y: 80 },
        { at_ms: 5_000, x: 500, y: 240 },
      ]
    : undefined;
  return { ...(keys ? { keys } : {}), ...(follow ? { follow, crop: { x: 100, y: 80, w: 640, h: 400 } } : {}) };
}
