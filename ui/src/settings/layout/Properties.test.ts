import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import Properties from './Properties.svelte';
import type { Block, ConnectionView } from '../../lib/types';

afterEach(cleanup);

const rect = (y: number) => ({ x: 0, y, w: 4, h: 2 });
const jira = {
  id: 'work', name: 'Work', kind: 'jira', kind_label: 'Jira', enabled: true, state: 'connected', secrets: [],
  describe: { summary: '', fields: [], lists: [{ id: 'projects', label: 'Projects', param: null }] },
  capabilities: { text: true, files: true, screenshot: true, video: true },
} as unknown as ConnectionView;

function dropdown(extra: Record<string, unknown> = {}): Block {
  return {
    id: 'dropdown-1', rect: rect(2), type: 'dropdown', title: 'Channel',
    source: { type: 'web_api', connection_id: 'work', options_id: 'projects' }, ...extra,
  } as Block;
}
const title: Block = { id: 'text-1', rect: rect(0), type: 'text', title: 'Title', placement: 'above', role: 'plain', default_text: '' } as Block;

function open(block: Block, blocks: Block[] = [block]) {
  return render(Properties, {
    block, blocks, rules: [], infoTypes: [], connections: [jira], variables: [], issues: [],
    onrect: () => true, onremove: () => {},
  });
}

describe('Properties: a dropdown fed by a service', () => {
  it('remembers the last choices unless that is switched off', async () => {
    const block = dropdown() as Block & { remember?: boolean };
    open(block);
    const box = screen.getByRole('switch', { name: 'Remember the last choices' }) as HTMLInputElement;
    expect(box.checked).toBe(true);
    await fireEvent.click(box);
    expect(block.remember).toBe(false);
  });

  it('can be left empty', async () => {
    const block = dropdown() as Block & { optional?: boolean };
    open(block);
    const box = screen.getByRole('switch', { name: 'Allow no choice' }) as HTMLInputElement;
    expect(box.checked).toBe(false);
    await fireEvent.click(box);
    expect(block.optional).toBe(true);
  });

  it('can be narrowed by a text block, and back to nothing', async () => {
    const block = dropdown() as Block & { narrow_by?: string | null };
    open(block, [title, block]);
    const pick = screen.getByLabelText('Narrow the list by') as HTMLSelectElement;
    await fireEvent.change(pick, { target: { value: 'text-1' } });
    expect(block.narrow_by).toBe('text-1');
    await fireEvent.change(pick, { target: { value: '' } });
    expect(block.narrow_by).toBeNull();
  });

  it('offers no narrowing when the layout has no text block', () => {
    open(dropdown());
    expect(screen.queryByLabelText('Narrow the list by')).toBeNull();
  });

  it('keeps these options off a list of Claude chats, which have their own order', () => {
    open(dropdown({ source: { type: 'claude_chats' } }));
    expect(screen.queryByRole('switch', { name: 'Remember the last choices' })).toBeNull();
  });
});
