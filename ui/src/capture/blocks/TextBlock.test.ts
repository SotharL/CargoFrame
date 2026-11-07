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

  it('inside the box: shown while empty, gone as soon as there is text, still the tooltip', async () => {
    render(TextBlock, { block: block('inside', 'prompt'), value: '' });
    const box = screen.getByRole('textbox', { name: 'Description' }) as HTMLTextAreaElement;
    expect(document.querySelector('.inner')?.textContent).toContain('Description');
    expect(document.querySelector('.inner')?.textContent).toContain('Prompt');

    await fireEvent.input(box, { target: { value: 'hello' } });
    expect(document.querySelector('.inner')).toBeNull();
    // No bubble on the box's edge (it was cut off at the top); the tooltip is the native one.
    expect(document.querySelector('.tab')).toBeNull();
    expect(document.querySelector('.field')?.getAttribute('title')).toBe('Description');
  });

  it('wears a small tag for the roles that mean something', () => {
    render(TextBlock, { block: block('above', 'zip_name'), value: '' });
    expect(document.querySelector('.role')?.textContent).toBe('Zip name');
    cleanup();
    render(TextBlock, { block: block('above', 'plain'), value: '' });
    expect(document.querySelector('.role')).toBeNull();
  });

  it('keeps the box named for screen readers either way', async () => {
    render(TextBlock, { block: block('inside'), value: 'already filled in' });
    expect(screen.getByRole('textbox', { name: 'Description' })).toBeTruthy();
  });
});
