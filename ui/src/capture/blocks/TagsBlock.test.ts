import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import TagsBlock from './TagsBlock.svelte';
import { MAX_TAGS, type Block } from '../../lib/types';

type Tags = Extract<Block, { type: 'tags' }>;
const block: Tags = { id: 'labels', type: 'tags', rect: { x: 0, y: 0, w: 4, h: 1 }, title: 'Labels', tags: [] };

/** The tags shown, in order. */
const chips = () => Array.from(document.querySelectorAll('.chip span')).map((s) => s.textContent);
const box = () => screen.getByRole('textbox', { name: 'Add a tag to Labels' }) as HTMLInputElement;

async function typeTag(text: string, key = 'Enter') {
  await fireEvent.input(box(), { target: { value: text } });
  await fireEvent.keyDown(box(), { key });
}

describe('TagsBlock', () => {
  afterEach(cleanup);

  it('adds a tag with Enter or a comma, each once, and takes the last out with Backspace', async () => {
    render(TagsBlock, { block, tags: ['crash'] });
    await typeTag('  gpu   driver ');
    await typeTag('Crash', ',');
    expect(chips()).toEqual(['crash', 'gpu driver']);
    await fireEvent.keyDown(box(), { key: 'Backspace' });
    expect(chips()).toEqual(['crash']);
    await fireEvent.click(screen.getByRole('button', { name: 'Remove crash' }));
    expect(chips()).toEqual([]);
  });

  it('makes several tags from a pasted list and stops at the limit', async () => {
    render(TagsBlock, { block, tags: Array.from({ length: MAX_TAGS - 2 }, (_, i) => `t${i}`) });
    const data = { getData: () => 'a, b\nc' };
    await fireEvent.paste(box(), { clipboardData: data });
    expect(chips()).toHaveLength(MAX_TAGS);
    expect(chips().slice(-2)).toEqual(['a', 'b']);
    expect(box().disabled).toBe(true);
    expect(screen.getByText(`${MAX_TAGS}/${MAX_TAGS}`)).toBeTruthy();
  });

  it('offers the last tags used that are not there yet, and the service’s tags that match', async () => {
    render(TagsBlock, {
      block,
      tags: ['ui'],
      recent: [{ value: 'ui', label: 'ui' }, { value: 'perf', label: 'perf' }],
      suggest: async () => [
        { value: 'gpu', label: 'gpu' },
        { value: 'gpu-driver', label: 'gpu-driver' },
        { value: 'audio', label: 'audio' },
      ],
    });
    const last = screen.getByRole('group', { name: 'Last tags used' });
    expect(last.textContent).toContain('perf');
    expect(last.textContent).not.toContain('ui');
    await fireEvent.click(screen.getByRole('button', { name: 'perf' }));
    expect(chips()).toEqual(['ui', 'perf']);
    await fireEvent.input(box(), { target: { value: 'gp' } });
    await waitFor(() => expect(screen.getByRole('group', { name: 'Tags of the service' })).toBeTruthy());
    await fireEvent.click(screen.getByRole('button', { name: 'gpu-driver' }));
    expect(chips()).toEqual(['ui', 'perf', 'gpu-driver']);
  });

  it('says how many tags the capture had beyond the limit', () => {
    render(TagsBlock, { block, tags: [], over: 3 });
    expect(screen.getByText(/3 more tags were left out/)).toBeTruthy();
  });
});
