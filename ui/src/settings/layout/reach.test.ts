import { describe, expect, it } from 'vitest';
import { blockName, lineFor } from './reach';
import type { Block } from '../../lib/types';

type Button = Extract<Block, { type: 'sharing_button' }>;
const button: Button = { id: 'b', type: 'sharing_button', rect: { x: 0, y: 0, w: 1, h: 1 }, label: '', connection_id: null, prompt_block_id: null, mappings: [], on_success: 'keep_open' };

describe('lineFor', () => {
  it('says how each sent block is used', () => {
    expect(lineFor({ block_id: 'a', state: 'sent', role: 'screenshot' }, button, 'jira').text).toBe('Sent as the screenshot');
    expect(lineFor({ block_id: 'a', state: 'sent', role: 'prompt' }, button, 'jira').text).toBe('Sent as the prompt');
    expect(lineFor({ block_id: 'a', state: 'sent', role: 'zip_name' }, button, null).text).toBe('Names the saved zip');
    expect(lineFor({ block_id: 'a', state: 'sent', role: 'files' }, button, 'jira').text).toBe('Sent as files');
    expect(lineFor({ block_id: 'a', state: 'sent', role: 'field', field: 'priority' }, button, 'jira').text).toBe('Sent in the “priority” field');
  });

  it('says a sent video is sent as the video, and names the block Video', () => {
    expect(lineFor({ block_id: 'a', state: 'sent', role: 'video' }, button, 'slack').text).toBe('Sent as the video');
    expect(blockName({ id: 'clip', type: 'video', rect: { x: 0, y: 0, w: 1, h: 1 } })).toBe('Video');
  });

  it('says text files go into the message only for a Claude button set that way', () => {
    const inline: Button = { ...button, send: { screenshot: true, files: 'inline', inline_limit_kb: 64 } };
    const r = { block_id: 'a', state: 'sent', role: 'files' } as const;
    expect(lineFor(r, inline, 'claude').text).toContain('inside the message');
    expect(lineFor(r, inline, 'jira').text).toBe('Sent as files');
    expect(lineFor(r, button, 'claude').text).toBe('Sent as files');
  });

  it('says the chat dropdown picks the chat to open when the destination is Claude', () => {
    expect(lineFor({ block_id: 'a', state: 'sent', role: 'field', field: 'chat' }, button, 'claude').text).toBe('Picks the chat to open');
  });

  it('uses a different icon for what is not sent, and quotes the reason it cannot be', () => {
    const cannot = lineFor({ block_id: 'a', state: 'cannot_send', why: 'No files on a post' }, button, 'reddit');
    expect(cannot).toEqual({ state: 'cannot_send', icon: '✕', text: 'Cannot be sent: No files on a post' });
    expect(lineFor({ block_id: 'a', state: 'switched_off' }, button, 'reddit').icon).toBe('–');
    expect(lineFor({ block_id: 'a', state: 'not_used' }, button, 'reddit').icon).toBe('–');
  });
});

describe('blockName', () => {
  it('names blocks the way the canvas does', () => {
    expect(blockName({ id: 's', type: 'screenshot', rect: { x: 0, y: 0, w: 1, h: 1 } })).toBe('Screenshot');
    expect(blockName(button)).toBe('Share');
  });
});
