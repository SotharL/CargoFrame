import { describe, expect, it } from 'vitest';
import { collectShare, promptIsEmpty } from './collect';
import { demoSession } from './demo';
import { initialValues } from './values';
import type { Block } from '../lib/types';

type Sharing = Extract<Block, { type: 'sharing_button' }>;

describe('collectShare', () => {
  it('sends the prompt bound to the button and only blocks that exist', async () => {
    const view = await demoSession();
    const button = view.layout!.blocks.find((b) => b.id === 'send') as Sharing;
    const bound: Sharing = { ...button, connection_id: 'claude', prompt_block_id: 'prompt' };
    const values = initialValues(view);
    values.texts['gone'] = 'stale';
    values.texts['prompt'] = 'Why does it crash?';
    const req = collectShare(view, bound, values, true);
    expect(req.prompt).toBe('Why does it crash?');
    expect(req.texts['gone']).toBeUndefined();
    expect(req.texts['note']).toContain('Seen on');
    expect(req.connection_id).toBe('claude');
    expect(req.image_edited).toBe(true);
    expect(req.button_id).toBe('send');
  });

  it('carries the part of the recording that was kept, and only for a recording', async () => {
    const view = await demoSession();
    const button = view.layout!.blocks.find((b) => b.id === 'send') as Sharing;
    const trim = { start_ms: 1000, end_ms: 4000 };
    // a screenshot capture has no recording to cut
    expect(collectShare(view, button, initialValues(view), false, trim).clip_edit).toBeNull();
    view.video = { width: 1280, height: 720, duration_ms: 9_500, bytes: 1024 };
    expect(collectShare(view, button, initialValues(view), false, trim).clip_edit).toEqual(trim);
    expect(collectShare(view, button, initialValues(view), false).clip_edit).toBeNull();
  });

  it('sends the boxes that are ticked for the button, and only blocks that exist', async () => {
    const view = await demoSession();
    const button = view.layout!.blocks.find((b) => b.id === 'send') as Sharing;
    const values = initialValues(view);
    expect(collectShare(view, button, values, false).send_blocks).toBeNull();
    const ticks = { send: ['shot', 'prompt', 'gone'], other: ['note'] };
    expect(collectShare(view, button, values, false, null, ticks).send_blocks).toEqual(['shot', 'prompt']);
    // a button with no boxes of its own sends what the layout says
    expect(collectShare(view, { ...button, id: 'third' }, values, false, null, ticks).send_blocks).toBeNull();
  });

  it('does not ask for a prompt whose box was cleared', async () => {
    const view = await demoSession();
    const button = view.layout!.blocks.find((b) => b.id === 'send') as Sharing;
    const bound: Sharing = { ...button, prompt_block_id: 'prompt' };
    const values = initialValues(view);
    values.texts['prompt'] = '';
    const ticked = collectShare(view, bound, values, false, null, { send: ['shot', 'prompt'] });
    expect(promptIsEmpty(ticked)).toBe(true);
    const cleared = collectShare(view, bound, values, false, null, { send: ['shot'] });
    expect(cleared.prompt).toBeNull();
    expect(promptIsEmpty(cleared)).toBe(false);
  });

  it('has no prompt when none is bound', async () => {
    const view = await demoSession();
    const button = { ...(view.layout!.blocks.find((b) => b.id === 'send') as Sharing), prompt_block_id: null };
    const req = collectShare(view, button, initialValues(view), false);
    expect(req.prompt).toBeNull();
    expect(promptIsEmpty(req)).toBe(false);
  });

  it('treats a blank bound prompt as empty', async () => {
    const view = await demoSession();
    const button = view.layout!.blocks.find((b) => b.id === 'send') as Sharing;
    const values = initialValues(view);
    values.texts['prompt'] = '   ';
    const req = collectShare(view, { ...button, prompt_block_id: 'prompt' }, values, false);
    expect(promptIsEmpty(req)).toBe(true);
  });
});
