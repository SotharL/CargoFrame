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

  it('has no prompt when none is bound', async () => {
    const view = await demoSession();
    const button = view.layout!.blocks.find((b) => b.id === 'send') as Sharing;
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
