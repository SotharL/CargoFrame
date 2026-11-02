import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render } from '@testing-library/svelte';
import Canvas from './Canvas.svelte';
import type { Block } from '../../lib/types';

const blocks: Block[] = [
  { id: 'desc', type: 'text', rect: { x: 0, y: 0, w: 3, h: 1 }, title: 'Description of the problem seen', placement: 'above', role: 'prompt', default_text: '' },
  { id: 'send', type: 'sharing_button', rect: { x: 3, y: 0, w: 3, h: 1 }, label: 'Send', connection_id: null, prompt_block_id: null, mappings: [], on_success: 'close_window' },
];

describe('Layout canvas', () => {
  afterEach(cleanup);

  it('a block one row high keeps its title and its small (type) on one line', () => {
    render(Canvas, { blocks, selected: null });
    const lines = [...document.querySelectorAll('.block .line')];
    expect(lines).toHaveLength(2);
    expect(lines[0]!.querySelector('.name')?.textContent).toBe('Description of the problem seen');
    expect(lines[0]!.querySelector('.kind')?.textContent).toBe('(text)');
    expect(lines[0]!.querySelector('.tag')?.textContent).toBe('Prompt');
    expect(lines[1]!.querySelector('.name')?.textContent).toBe('Send');
    expect(lines[1]!.querySelector('.kind')?.textContent).toBe('(sharing button)');
  });
});
