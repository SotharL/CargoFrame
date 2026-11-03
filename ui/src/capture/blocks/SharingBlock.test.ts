import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/svelte';
import SharingBlock from './SharingBlock.svelte';
import type { Block } from '../../lib/types';

type Button = Extract<Block, { type: 'sharing_button' }>;

const block: Button = {
  id: 'send',
  type: 'sharing_button',
  rect: { x: 0, y: 0, w: 4, h: 1 },
  label: 'Create ticket',
  connection_id: null,
  prompt_block_id: null,
  mappings: [],
  on_success: 'close_window',
};

describe('SharingBlock', () => {
  afterEach(cleanup);

  it('uses the layout label when it sends', async () => {
    const onshare = vi.fn();
    render(SharingBlock, { block, enabled: true, reasons: [], onshare });
    await fireEvent.click(screen.getByRole('button', { name: 'Create ticket' }));
    expect(onshare).toHaveBeenCalled();
  });

  it('reads Save locally, whatever the layout label, when there is no active connection', () => {
    render(SharingBlock, {
      block, enabled: true, reasons: [], mode: 'save_locally',
      note: 'No active connection: a zip is saved in /home/me/zips', onshare: () => {},
    });
    expect(screen.getByRole('button', { name: 'Save locally' })).toBeTruthy();
    expect(screen.queryByText('Create ticket')).toBeNull();
    expect(screen.getByText('No active connection: a zip is saved in /home/me/zips')).toBeTruthy();
  });

  it('is disabled and says why when the folder for saved zips is not set', () => {
    render(SharingBlock, {
      block, enabled: false, mode: 'save_locally', note: null,
      reasons: ['Set a “Folder for saved zips” in Settings, General, Safety to save locally'],
      onshare: () => {},
    });
    const button = screen.getByRole('button', { name: 'Save locally' }) as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    expect(screen.getByText(/Folder for saved zips/)).toBeTruthy();
  });

  it('shows progress while saving', () => {
    render(SharingBlock, {
      block, enabled: false, reasons: [], mode: 'save_locally', sending: true, onshare: () => {},
    });
    expect(screen.getByRole('button', { name: 'Saving…' })).toBeTruthy();
  });

  it('says what a click carries, and the Claude chat that opens', () => {
    render(SharingBlock, {
      block, enabled: true, reasons: [], kind: 'claude', opening: 'Release notes',
      parts: ['Screenshot (edited)', '3 files (12 KB)', 'Prompt'], onshare: () => {},
    });
    expect(screen.getByText('Opening: Release notes')).toBeTruthy();
    expect(screen.getByText('Will send: Screenshot (edited) · 3 files (12 KB) · Prompt')).toBeTruthy();
  });

  it('says "Will save" when it saves a zip here, and shows nothing when nothing would go', () => {
    const { unmount } = render(SharingBlock, {
      block, enabled: true, reasons: [], mode: 'save_locally', parts: ['Screenshot'], onshare: () => {},
    });
    expect(screen.getByText('Will save: Screenshot')).toBeTruthy();
    unmount();
    render(SharingBlock, { block, enabled: true, reasons: [], parts: [], onshare: () => {} });
    expect(screen.queryByText(/Will send/)).toBeNull();
  });

  it('takes its destination colours and names the destination on hover', () => {
    render(SharingBlock, { block, enabled: true, reasons: [], kind: 'reddit', onshare: () => {} });
    const button = screen.getByRole('button', { name: 'Create ticket' });
    expect(button.title).toBe('Sends to Reddit');
    expect(button.parentElement!.getAttribute('style')).toContain('--kind: var(--kind-reddit)');
  });

  it('tells the window when the pointer or focus is on the button', async () => {
    const onhover = vi.fn();
    render(SharingBlock, { block, enabled: true, reasons: [], onshare: () => {}, onhover });
    const button = screen.getByRole('button', { name: 'Create ticket' });
    await fireEvent.mouseEnter(button);
    await fireEvent.mouseLeave(button);
    await fireEvent.focus(button);
    await fireEvent.blur(button);
    expect(onhover.mock.calls.map((c) => c[0])).toEqual([true, false, true, false]);
  });

  it('hides the lines while it is disabled or sending', () => {
    render(SharingBlock, {
      block, enabled: false, reasons: ['Pick a chat'], parts: ['Prompt'], opening: 'Bug triage', onshare: () => {},
    });
    expect(screen.queryByText(/Will send/)).toBeNull();
    expect(screen.queryByText(/Opening/)).toBeNull();
    expect(screen.getByText('Pick a chat')).toBeTruthy();
  });
});
