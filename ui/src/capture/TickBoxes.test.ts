import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import TickBoxes from './TickBoxes.svelte';
import type { TickBox } from './ticks';

const box = (over: Partial<TickBox> = {}): TickBox => ({
  buttonId: 'send', label: 'Create ticket', kind: 'jira', checked: true, locked: false, disabled: false,
  title: 'Create ticket sends this', badge: null, ...over,
});

describe('TickBoxes', () => {
  afterEach(cleanup);

  it('shows one box per button, named for what it does, with a ✓ when it is ticked', () => {
    render(TickBoxes, {
      boxes: [box(), box({ buttonId: 'b', label: 'Post', kind: 'reddit', checked: false, title: 'Post leaves this out' })],
      onchange: () => {},
    });
    const boxes = screen.getAllByRole('checkbox') as HTMLInputElement[];
    expect(boxes.map((b) => [b.getAttribute('aria-label'), b.checked])).toEqual([
      ['Create ticket sends this', true],
      ['Post leaves this out', false],
    ]);
    // the ✓ is text, so colour is never the only sign
    expect(screen.getAllByText('✓')).toHaveLength(1);
  });

  it('draws nothing for a block with no boxes', () => {
    const { container } = render(TickBoxes, { boxes: [], onchange: () => {} });
    expect(container.querySelector('.ticks')).toBeNull();
  });

  it('tells the window which button was ticked or cleared', async () => {
    const onchange = vi.fn();
    render(TickBoxes, { boxes: [box({ buttonId: 'a', checked: false }), box({ buttonId: 'b' })], onchange });
    const [a, b] = screen.getAllByRole('checkbox');
    await fireEvent.click(a!);
    await fireEvent.click(b!);
    expect(onchange.mock.calls).toEqual([['a', true], ['b', false]]);
  });

  it('cannot be changed when the box is blocked or the button needs the block', async () => {
    const onchange = vi.fn();
    render(TickBoxes, {
      boxes: [box({ buttonId: 'a', checked: false, disabled: true }), box({ buttonId: 'b', locked: true })],
      onchange,
    });
    const [a, b] = screen.getAllByRole('checkbox') as HTMLInputElement[];
    // a disabled box takes no click in a browser; jsdom does not model that, so the flag is what counts
    expect(a!.disabled).toBe(true);
    expect(b!.disabled).toBe(true);
    expect(onchange).not.toHaveBeenCalled();
  });

  it('lights up the button of the box under the pointer or the keyboard focus, and its number', async () => {
    const onhover = vi.fn();
    render(TickBoxes, { boxes: [box({ badge: '2' })], active: 'send', onchange: () => {}, onhover });
    const label = screen.getByRole('checkbox').closest('label')!;
    expect(label.classList.contains('lit')).toBe(true);
    expect(label.textContent).toContain('2');
    await fireEvent.mouseEnter(label);
    await fireEvent.mouseLeave(label);
    await fireEvent.focus(screen.getByRole('checkbox'));
    await fireEvent.blur(screen.getByRole('checkbox'));
    expect(onhover.mock.calls.map((c) => c[0])).toEqual(['send', null, 'send', null]);
  });

  it('puts the destination colour on each box and the reason in the tooltip', () => {
    render(TickBoxes, { boxes: [box({ kind: 'reddit', disabled: true, checked: false, title: 'Post cannot send this: no files' })], onchange: () => {} });
    const label = screen.getByRole('checkbox').closest('label')!;
    expect(label.getAttribute('style')).toContain('--kind: var(--kind-reddit)');
    expect(label.getAttribute('title')).toBe('Post cannot send this: no files');
  });
});
