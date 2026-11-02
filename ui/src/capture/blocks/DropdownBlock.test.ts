import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import DropdownBlock from './DropdownBlock.svelte';
import type { Block, DropdownState } from '../../lib/types';

type Dropdown = Extract<Block, { type: 'dropdown' }>;
const block: Dropdown = {
  id: 'labels', type: 'dropdown', rect: { x: 0, y: 0, w: 4, h: 2 }, title: 'Label',
  source: { type: 'web_api', connection_id: 'work', options_id: 'labels' },
};
const info = (n: number): DropdownState => ({
  status: 'ready',
  options: Array.from({ length: n }, (_, i) => ({ value: `label-${i}`, label: `Label ${i}`, group: null })),
  selected: null,
  message: null,
});

describe('DropdownBlock', () => {
  afterEach(cleanup);

  it('a short list stays a plain drop down', () => {
    render(DropdownBlock, { block, info: info(5), value: null });
    expect(screen.getByRole('combobox', { name: 'Label' }).tagName).toBe('SELECT');
  });

  it('a long list can be searched', async () => {
    render(DropdownBlock, { block, info: info(40), value: null });
    const box = screen.getByRole('combobox', { name: 'Label' });
    expect(box.tagName).toBe('INPUT');
    await fireEvent.focus(box);
    await fireEvent.input(box, { target: { value: 'label 3' } });
    // Label 3, and Label 30 to 39.
    expect(screen.getAllByRole('option')).toHaveLength(11);
    await fireEvent.pointerDown(screen.getByRole('option', { name: 'Label 34' }));
    expect((box as HTMLInputElement).value).toBe('Label 34');
  });
});
