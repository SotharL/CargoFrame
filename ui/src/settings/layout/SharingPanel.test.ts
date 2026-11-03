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
    expect(screen.getByText(/Sent as the prompt/)).toBeTruthy();
    expect(screen.getByText(/Cannot be sent: Reddit cannot take files on a post/)).toBeTruthy();
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
