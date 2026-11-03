import { describe, expect, it } from 'vitest';
import { demoSession } from './demo';
import { demoCapabilities, demoReach } from '../lib/demoReach';
import { initialValues } from './values';
import { chatToOpen, marksFor, willSend } from './willSend';
import type { Block, ServiceKind, SessionView } from '../lib/types';

type Button = Extract<Block, { type: 'sharing_button' }>;

/** The demo capture, with its one button pointed at `kind` and its reach worked out again. */
async function viewFor(kind: ServiceKind | null, change: (b: Button) => void = () => {}): Promise<{ view: SessionView; button: Button }> {
  const view = await demoSession();
  const button = view.layout!.blocks.find((b): b is Button => b.type === 'sharing_button')!;
  // Start from a bare button: each test says what it maps.
  button.prompt_block_id = null;
  button.mappings = [];
  change(button);
  const caps = demoCapabilities(kind ?? 'jira');
  view.sharing[button.id] = {
    enabled: true, reasons: [], mode: kind ? 'send' : 'save_locally', note: null, kind,
    reach: demoReach(view.layout!, button, kind ? caps : demoCapabilities('jira'), kind),
  };
  return { view, button };
}

describe('willSend', () => {
  it('lists the picture, the ticked files, the prompt and the filled fields, in that order', async () => {
    const { view, button } = await viewFor('jira', (b) => {
      b.prompt_block_id = 'prompt';
      b.mappings = [{ field: 'description', source: { kind: 'block', value: 'note' } }, { field: 'project', source: { kind: 'block', value: 'proj' } }];
    });
    view.dropdowns.proj!.selected = 'CF';
    const values = initialValues(view);
    expect(willSend(view, button, values, false)).toEqual(['Screenshot', '1 file (18 KB)', 'Prompt', 'Note', 'Jira project']);
  });

  it('lists the recording of a video capture, with its length and size, where the picture would be', async () => {
    const { view, button } = await viewFor('slack', (b) => { b.prompt_block_id = 'prompt'; });
    view.layout!.blocks = view.layout!.blocks.filter((b) => b.type !== 'screenshot');
    view.layout!.blocks.unshift({ id: 'clip', type: 'video', rect: { x: 0, y: 0, w: 8, h: 8 } });
    view.video = { width: 1280, height: 720, duration_ms: 9_500, bytes: 3 * 1024 * 1024 };
    view.sharing[button.id]!.reach = demoReach(view.layout!, button, demoCapabilities('slack'), 'slack');
    expect(willSend(view, button, initialValues(view), false)[0]).toBe('Video (9.5 s, 3.0 MB)');
    // a recording that was cut says how much of it goes
    expect(willSend(view, button, initialValues(view), false, { start_ms: 1_000, end_ms: 4_000, crop: null })[0]).toBe('Video (3.0 s of 9.5 s, cut)');
    expect(willSend(view, button, initialValues(view), false, { start_ms: 2_000, end_ms: null })[0]).toBe('Video (7.5 s of 9.5 s, cut)');
    const crop = { x: 0, y: 0, w: 640, h: 360 };
    expect(willSend(view, button, initialValues(view), false, { start_ms: 0, end_ms: null, crop })[0]).toBe('Video (9.5 s, cropped)');
    expect(willSend(view, button, initialValues(view), false, { start_ms: 1_000, end_ms: 4_000, crop })[0]).toBe('Video (3.0 s of 9.5 s, cut and cropped)');
    expect(willSend(view, button, initialValues(view), false, { start_ms: 0, end_ms: null, mark: true })[0]).toBe('Video (9.5 s, marked)');
    expect(willSend(view, button, initialValues(view), false, { start_ms: 0, end_ms: null, crop, mark: true })[0]).toBe('Video (9.5 s, cropped and marked)');
    expect(willSend(view, button, initialValues(view), false, { start_ms: 1_000, end_ms: 4_000, crop, mark: true })[0]).toBe('Video (3.0 s of 9.5 s, cut, cropped and marked)');
    // where video cannot go, it is not listed
    view.sharing[button.id]!.reach = demoReach(view.layout!, button, demoCapabilities('teams'), 'teams');
    expect(willSend(view, button, initialValues(view), false).join()).not.toContain('Video');
  });

  it('says when the picture was edited and counts every ticked file', async () => {
    const { view, button } = await viewFor('jira', (b) => { b.prompt_block_id = 'prompt'; });
    const values = initialValues(view);
    values.files.logs = ['app.log', 'logs/debug.log'];
    const parts = willSend(view, button, values, true);
    expect(parts[0]).toBe('Screenshot (edited)');
    expect(parts[1]).toBe('2 files (2.3 MB)');
  });

  it('leaves out files nobody ticked, text with nothing in it and a screenshot the button switches off', async () => {
    const { view, button } = await viewFor('jira', (b) => {
      b.prompt_block_id = 'prompt';
      b.send = { screenshot: false, files: 'attach', inline_limit_kb: 64 };
      b.mappings = [{ field: 'description', source: { kind: 'block', value: 'note' } }];
    });
    const values = initialValues(view);
    values.files.logs = [];
    values.texts.note = '   ';
    expect(willSend(view, button, values, false)).toEqual(['Prompt']);
  });

  it('leaves out what the destination cannot take', async () => {
    const { view, button } = await viewFor('reddit', (b) => { b.prompt_block_id = 'prompt'; });
    const values = initialValues(view);
    // Reddit takes no files: they are marked ✕ on the block, and not promised here.
    expect(willSend(view, button, values, false)).toEqual(['Screenshot', 'Prompt']);
  });

  it('says text files are written into the message when the Claude button is set that way', async () => {
    const { view, button } = await viewFor('claude', (b) => {
      b.prompt_block_id = 'prompt';
      b.send = { screenshot: true, files: 'inline', inline_limit_kb: 64 };
    });
    const parts = willSend(view, button, initialValues(view), false);
    expect(parts).toContain('1 file (18 KB), text ones as text');
  });

  it('does not list the chat as content: it is the place that opens', async () => {
    const { view, button } = await viewFor('claude', (b) => { b.prompt_block_id = 'prompt'; });
    expect(willSend(view, button, initialValues(view), false)).not.toContain('Claude chat');
  });
});

describe('chatToOpen', () => {
  it('is null for any destination but Claude', async () => {
    const { view, button } = await viewFor('jira');
    expect(chatToOpen(view, button, initialValues(view))).toBeNull();
  });

  it('names the chat that is picked, and falls back to a new chat', async () => {
    const { view, button } = await viewFor('claude', (b) => { b.prompt_block_id = 'prompt'; });
    const values = initialValues(view);
    expect(chatToOpen(view, button, values)).toBe('Release notes');
    values.dropdowns.chat = 'new';
    expect(chatToOpen(view, button, values)).toBe('a new chat');
    values.dropdowns.chat = null;
    expect(chatToOpen(view, button, values)).toBe('a new chat');
  });
});

describe('marksFor', () => {
  it('marks a block as sent by the button that sends it', async () => {
    const { view } = await viewFor('jira', (b) => { b.prompt_block_id = 'prompt'; });
    const m = marksFor(view, 'shot');
    expect(m.marks.map((x) => [x.label, x.state, x.kind])).toEqual([['Share', 'sent', 'jira']]);
    expect(m.notSent).toBe(false);
  });

  it('marks a block the destination cannot take, with the reason, and says it is not sent', async () => {
    const { view } = await viewFor('reddit', (b) => { b.prompt_block_id = 'prompt'; });
    const m = marksFor(view, 'logs');
    expect(m.marks).toHaveLength(1);
    expect(m.marks[0]).toMatchObject({ state: 'cannot_send', why: 'Reddit cannot take files on a post' });
    expect(m.notSent).toBe(true);
    expect(m.notSentWhy).toContain('cannot send it');
  });

  it('gives one plain "not sent" for a block no button uses', async () => {
    const { view } = await viewFor('jira', (b) => { b.prompt_block_id = 'prompt'; });
    const m = marksFor(view, 'note');
    expect(m.marks).toEqual([]);
    expect(m.notSent).toBe(true);
    expect(m.notSentWhy).toContain('no field for it');
  });

  it('says a switched-off block is left out by the button', async () => {
    const { view } = await viewFor('jira', (b) => {
      b.prompt_block_id = 'prompt';
      b.send = { screenshot: false, files: 'attach', inline_limit_kb: 64 };
    });
    const m = marksFor(view, 'shot');
    expect(m.notSent).toBe(true);
    expect(m.notSentWhy).toBe('Share is set to leave it out');
  });

  it('names a button that saves a zip here "Save locally", with no destination colour', async () => {
    const { view } = await viewFor(null, (b) => { b.prompt_block_id = 'prompt'; });
    const m = marksFor(view, 'shot');
    expect(m.marks[0]).toMatchObject({ label: 'Save locally', kind: null, state: 'sent' });
  });

  it('marks nothing when the layout has no sharing button, or no reach was worked out', async () => {
    const { view } = await viewFor('jira');
    view.sharing = {};
    expect(marksFor(view, 'shot')).toEqual({ marks: [], notSent: false, notSentWhy: null });
  });
});
