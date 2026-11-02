import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import TextBlock from './TextBlock.svelte';
import type { Block } from '../../lib/types';

type Text = Extract<Block, { type: 'text' }>;
const block = (placement: Text['placement'], role: Text['role'] = 'plain'): Text => ({
  id: 'note', type: 'text', rect: { x: 0, y: 0, w: 4, h: 2 }, title: 'Description', placement, role, default_text: '',
});

describe('TextBlock title', () => {
  afterEach(cleanup);

  it('sits above the box when asked to', () => {
    render(TextBlock, { block: block('above'), value: '' });
    expect(screen.getByLabelText('Description')).toBeTruthy();
    expect(document.querySelector('.inner')).toBeNull();
  });

  it('inside the box: shown while empty, gone as soon as there is text, back as a hover tab', async () => {
    render(TextBlock, { block: block('inside', 'prompt'), value: '' });
    const box = screen.getByRole('textbox', { name: 'Description' }) as HTMLTextAreaElement;
    expect(document.querySelector('.inner')?.textContent).toContain('Description');
    expect(document.querySelector('.inner')?.textContent).toContain('Prompt');
    expect(document.querySelector('.tab')).toBeNull();

    await fireEvent.input(box, { target: { value: 'hello' } });
    expect(document.querySelector('.inner')).toBeNull();
    // The title still exists for hovering: a tab on the edge, and the native tooltip.
    expect(document.querySelector('.tab')?.textContent).toContain('Description');
    expect(document.querySelector('.field')?.getAttribute('title')).toBe('Description');
  });

  it('keeps the box named for screen readers either way', async () => {
    render(TextBlock, { block: block('inside'), value: 'already filled in' });
    expect(screen.getByRole('textbox', { name: 'Description' })).toBeTruthy();
  });
});
