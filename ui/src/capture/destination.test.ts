import { describe, expect, it } from 'vitest';
import { demoSession } from './demo';
import { nowhereToPost } from './destination';
import { collectShare } from './collect';
import { initialValues } from './values';
import type { Block, SessionView } from '../lib/types';

type Button = Extract<Block, { type: 'sharing_button' }>;

async function discordView(options: number, optional = false) {
  const view: SessionView = await demoSession();
  const layout = view.layout!;
  layout.blocks = [
    { id: 'channel', type: 'dropdown', rect: { x: 0, y: 0, w: 4, h: 1 }, title: 'Channel', optional,
      source: { type: 'web_api', connection_id: 'd', options_id: 'channels' } },
    { id: 'send', type: 'sharing_button', rect: { x: 0, y: 1, w: 4, h: 1 }, label: 'Post', connection_id: 'd',
      prompt_block_id: null, mappings: [{ field: 'channel', source: { kind: 'block', value: 'channel' } }],
      on_success: 'close_window' } as Button & { id: string; rect: object },
  ] as Block[];
  view.dropdowns = {
    channel: {
      status: 'ready', selected: null, message: null,
      options: Array.from({ length: options }, (_, i) => ({ value: `c${i}`, label: `#c${i}`, group: null })),
    },
  };
  const button = layout.blocks[1] as Button;
  return { view, button };
}

describe('nowhere to post', () => {
  it('is true when the list the button needs came back empty', async () => {
    const { view, button } = await discordView(0);
    expect(nowhereToPost(view, button)).toBe(true);
    expect(collectShare(view, button, initialValues(view), false).save_here).toBe(true);
  });

  it('is false when there is somewhere to post, and the request says nothing about it', async () => {
    const { view, button } = await discordView(2);
    expect(nowhereToPost(view, button)).toBe(false);
    expect(collectShare(view, button, initialValues(view), false).save_here).toBeUndefined();
  });

  it('is false while the list loads, when it failed, and for a list that may stay empty', async () => {
    const { view, button } = await discordView(0);
    view.dropdowns['channel']!.status = 'loading';
    expect(nowhereToPost(view, button)).toBe(false);
    view.dropdowns['channel']!.status = 'needs_connection';
    expect(nowhereToPost(view, button)).toBe(false);
    const optional = await discordView(0, true);
    expect(nowhereToPost(optional.view, optional.button)).toBe(false);
  });
});
