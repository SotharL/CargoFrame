import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import SharingPanel from './SharingPanel.svelte';
import Harness from './SharingPanel.harness.svelte';
import { demoCapabilities, demoReach, LOCAL_CAPABILITIES } from '../../lib/demoReach';
import type { Block, Layout, ServiceKind } from '../../lib/types';

type Button = Extract<Block, { type: 'sharing_button' }>;

function setup(kind: ServiceKind | null, patch: Partial<Button> = {}) {
  const button: Button = {
    id: 'send', type: 'sharing_button', rect: { x: 4, y: 2, w: 4, h: 1 }, label: 'Go', connection_id: kind ? 'c' : null,
    prompt_block_id: 'prompt', mappings: [], on_success: 'keep_open', ...patch,
  };
  const blocks: Block[] = [
    { id: 'shot', type: 'screenshot', rect: { x: 0, y: 0, w: 4, h: 2 } },
    { id: 'logs', type: 'files', rect: { x: 4, y: 0, w: 4, h: 2 }, title: 'Logs', include: [], preselect: [], look_in: { folders: ['project'], subfolder: '' } },
    { id: 'prompt', type: 'text', rect: { x: 0, y: 2, w: 4, h: 1 }, title: 'What happened', placement: 'above', role: 'prompt', default_text: '' },
    button,
  ];
  const layout: Layout = { id: 'l', name: 'L', blocks };
  const caps = kind ? demoCapabilities(kind) : LOCAL_CAPABILITIES;
  return { button, blocks, reach: demoReach(layout, button, caps, kind) };
}

/** Renders the panel the way the page holds it, and returns what the button looks like after edits. */
function live(s: ReturnType<typeof setup>, kind: ServiceKind | null) {
  const seen: { now: Button } = { now: s.button };
  render(Harness, { initial: s.button, blocks: s.blocks, kind, reach: s.reach, onchange: (b: Button) => (seen.now = b) });
  return seen;
}

describe('SharingPanel', () => {
  afterEach(cleanup);

  it('lists what the destination can take, with a catch or a no explained', () => {
    const { button, blocks, reach } = setup('reddit');
    render(SharingPanel, { block: button, blocks, kind: 'reddit', reach });
    expect(screen.getByText('Reddit can take')).toBeTruthy();
    expect(screen.getAllByText('Reddit cannot take files on a post').length).toBeGreaterThan(0);
    const files = screen.getByText('Files').closest('li')!;
    expect(files.classList.contains('no')).toBe(true);
    expect(files.textContent).toContain('✕');
  });

  it('says what this button does with each block', () => {
    const { button, blocks, reach } = setup('reddit');
    render(SharingPanel, { block: button, blocks, kind: 'reddit', reach });
    expect(screen.getByText(/Sent as the screenshot/)).toBeTruthy();
    // a text block is a choice, and the choice shows what the button does now
    const prompt = screen.getByLabelText('What happened: what this button does with it') as HTMLSelectElement;
    expect(prompt.value).toBe('@prompt');
    expect(screen.getByText(/Cannot be sent: Reddit cannot take files on a post/)).toBeTruthy();
  });

  describe('choosing where a block goes', () => {
    const fields = [{ key: 'summary', label: 'Summary' }, { key: 'description', label: 'Description' }];
    function withNote(kind: ServiceKind | null) {
      const s = setup(kind);
      s.blocks.splice(3, 0, { id: 'note', type: 'text', rect: { x: 0, y: 3, w: 4, h: 1 }, title: 'Note', placement: 'above', role: 'plain', default_text: '' });
      const layout: Layout = { id: 'l', name: 'L', blocks: s.blocks };
      return { ...s, reach: demoReach(layout, s.button, kind ? demoCapabilities(kind) : LOCAL_CAPABILITIES, kind) };
    }
    function liveWith(s: ReturnType<typeof withNote>, kind: ServiceKind | null) {
      const seen: { now: Button } = { now: s.button };
      render(Harness, { initial: s.button, blocks: s.blocks, kind, reach: s.reach, fields, onchange: (b: Button) => (seen.now = b) });
      return seen;
    }

    it('lists not sent, the prompt and the fields of the destination', () => {
      liveWith(withNote('jira'), 'jira');
      const note = screen.getByLabelText('Note: what this button does with it') as HTMLSelectElement;
      expect([...note.options].map((o) => o.textContent)).toEqual(['Not sent', 'Summary', 'Description']);
      const prompt = screen.getByLabelText('What happened: what this button does with it') as HTMLSelectElement;
      expect([...prompt.options].map((o) => o.textContent)).toEqual(['Not sent', 'The prompt', 'Summary', 'Description']);
    });

    it('sends a block in a field of the destination, and stops sending it', async () => {
      const seen = liveWith(withNote('jira'), 'jira');
      const note = screen.getByLabelText('Note: what this button does with it');
      await fireEvent.change(note, { target: { value: 'description' } });
      await waitFor(() => expect(seen.now.mappings).toEqual([{ field: 'description', source: { kind: 'block', value: 'note' } }]));
      await fireEvent.change(note, { target: { value: '' } });
      await waitFor(() => expect(seen.now.mappings).toEqual([]));
    });

    it('moves the prompt to another block', async () => {
      const seen = liveWith(withNote('jira'), 'jira');
      await fireEvent.change(screen.getByLabelText('What happened: what this button does with it'), { target: { value: '' } });
      await waitFor(() => expect(seen.now.prompt_block_id).toBeNull());
    });

    it('does not offer a field that another block already fills, and says which block', async () => {
      const s = withNote('jira');
      s.button.mappings = [{ field: 'summary', source: { kind: 'block', value: 'note' } }];
      s.reach = demoReach({ id: 'l', name: 'L', blocks: s.blocks }, s.button, demoCapabilities('jira'), 'jira');
      liveWith(s, 'jira');
      const prompt = screen.getByLabelText('What happened: what this button does with it') as HTMLSelectElement;
      const summary = [...prompt.options].find((o) => o.value === 'summary')!;
      expect(summary.textContent).toBe('Summary (used by Note)');
      expect(summary.disabled).toBe(true);
    });

    it('is switched off, with the reason, for a block the destination cannot take', () => {
      const s = withNote('custom');
      s.blocks.splice(3, 0, { id: 'proj', type: 'dropdown', rect: { x: 0, y: 4, w: 4, h: 1 }, title: 'Project', source: { type: 'claude_chats' } });
      s.button.mappings = [{ field: 'project', source: { kind: 'block', value: 'proj' } }];
      s.reach = demoReach({ id: 'l', name: 'L', blocks: s.blocks }, s.button, demoCapabilities('custom'), 'custom');
      liveWith(s, 'custom');
      // a Custom API takes text but has no lists to pick from
      expect((screen.getByLabelText('Note: what this button does with it') as HTMLSelectElement).disabled).toBe(false);
      expect((screen.getByLabelText('Project: what this button does with it') as HTMLSelectElement).disabled).toBe(true);
      expect(screen.getByText(/Cannot be sent: This connection has no lists to pick from/)).toBeTruthy();
    });

    it('is a plain line, not a choice, when the button only saves a zip here', () => {
      liveWith(withNote(null), null);
      expect(screen.queryByLabelText('Note: what this button does with it')).toBeNull();
    });
  });

  it('names the saved zip case when nothing is connected', () => {
    const { button, blocks, reach } = setup(null);
    render(SharingPanel, { block: button, blocks, kind: null, reach });
    expect(screen.getByText('Saving a zip on this computer keeps')).toBeTruthy();
  });

  it('turns the screenshot rule off and back on', async () => {
    const s = setup('jira');
    const seen = live(s, 'jira');
    const box = screen.getByRole('checkbox', { name: 'Send the screenshot' }) as HTMLInputElement;
    expect(box.checked).toBe(true);
    await fireEvent.click(box);
    await waitFor(() => expect(seen.now.send).toEqual({ screenshot: false, files: 'attach', inline_limit_kb: 64 }));
    await fireEvent.click(box);
    await waitFor(() => expect(seen.now.send?.screenshot).toBe(true));
  });

  it('greys out the screenshot rule when the destination cannot take a picture, and says why', () => {
    const { button, blocks, reach } = setup('custom');
    render(SharingPanel, { block: button, blocks, kind: 'custom', reach });
    const box = screen.getByRole('checkbox', { name: 'Send the screenshot' }) as HTMLInputElement;
    expect(box.disabled).toBe(true);
    expect(box.checked).toBe(false);
    expect(screen.getAllByText('The request has no multipart field for the screenshot').length).toBeGreaterThan(0);
  });

  it('offers to write text files into the message for Claude only, and then the size limit', async () => {
    const jira = setup('jira');
    const { unmount } = render(SharingPanel, { block: jira.button, blocks: jira.blocks, kind: 'jira', reach: jira.reach });
    expect(screen.queryByRole('option', { name: 'Write text files into the message' })).toBeNull();
    unmount();

    const claude = setup('claude');
    const seen = live(claude, 'claude');
    expect(screen.queryByLabelText('Largest text file to write in (KB)')).toBeNull();
    await fireEvent.change(screen.getByLabelText('Ticked files'), { target: { value: 'inline' } });
    await waitFor(() => expect(seen.now.send?.files).toBe('inline'));
    const limit = (await screen.findByLabelText('Largest text file to write in (KB)')) as HTMLInputElement;
    expect(limit.value).toBe('64');
  });

  it('keeps the size limit between 1 and the largest allowed', async () => {
    const s = setup('claude', { send: { screenshot: true, files: 'inline', inline_limit_kb: 64 } });
    const seen = live(s, 'claude');
    const limit = screen.getByLabelText('Largest text file to write in (KB)') as HTMLInputElement;
    await fireEvent.change(limit, { target: { value: '9999' } });
    await waitFor(() => expect(seen.now.send?.inline_limit_kb).toBe(512));
    await fireEvent.change(limit, { target: { value: '0' } });
    await waitFor(() => expect(seen.now.send?.inline_limit_kb).toBe(64));
    await fireEvent.change(limit, { target: { value: '-5' } });
    await waitFor(() => expect(seen.now.send?.inline_limit_kb).toBe(1));
  });

  it('disables the files rule and says why when the destination takes no files', () => {
    const { button, blocks, reach } = setup('reddit');
    render(SharingPanel, { block: button, blocks, kind: 'reddit', reach });
    const files = screen.getByLabelText('Ticked files') as HTMLSelectElement;
    expect(files.disabled).toBe(true);
    expect(files.value).toBe('none');
  });

  it('shows the connection\'s own table until the backend has answered', () => {
    const { button, blocks } = setup('jira');
    render(SharingPanel, { block: button, blocks, kind: 'jira', reach: undefined, fallback: demoCapabilities('jira') });
    expect(screen.getByText('Jira can take')).toBeTruthy();
    expect(screen.queryByText('What this button does with each block')).toBeNull();
  });
});

describe('SharingPanel for a video layout', () => {
  afterEach(cleanup);

  function videoSetup(kind: ServiceKind) {
    const s = setup(kind);
    s.blocks[0] = { id: 'clip', type: 'video', rect: { x: 0, y: 0, w: 4, h: 2 } };
    const layout: Layout = { id: 'l', name: 'L', blocks: s.blocks };
    return { ...s, reach: demoReach(layout, s.button, demoCapabilities(kind), kind) };
  }

  it('talks about the video, not the screenshot, and lets a destination that takes video have it', () => {
    live(videoSetup('slack'), 'slack');
    const box = screen.getByRole('checkbox', { name: 'Send the video' }) as HTMLInputElement;
    expect(box.disabled).toBe(false);
    expect(box.checked).toBe(true);
    expect(screen.queryByRole('checkbox', { name: 'Send the screenshot' })).toBeNull();
    expect(screen.getByText(/Sent as the video/)).toBeTruthy();
  });

  it('greys the switch out and says why where video cannot go', () => {
    live(videoSetup('claude'), 'claude');
    const box = screen.getByRole('checkbox', { name: 'Send the video' }) as HTMLInputElement;
    expect(box.disabled).toBe(true);
    expect(box.checked).toBe(false);
    expect(screen.getAllByText('Claude cannot watch a video').length).toBeGreaterThan(0);
  });
});
