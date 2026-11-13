import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import TabsBlock from './TabsBlock.svelte';
import type { TabsBlock as Tabs } from '../../lib/tabs';

const block: Tabs = { id: 'tabs', type: 'tabs', rect: { x: 0, y: 0, w: 4, h: 3 }, tabs: [{ name: 'Details' }, { name: 'Logs' }, { name: '' }] };
const children = createRawSnippet(() => ({ render: () => '<p>page</p>' }));

describe('TabsBlock', () => {
  afterEach(cleanup);

  it('names its tabs, an unnamed one by its number, and opens one on a click', async () => {
    render(TabsBlock, { block, children });
    const tabs = screen.getAllByRole('tab');
    expect(tabs.map((t) => t.textContent)).toEqual(['Details', 'Logs', 'Tab 3']);
    expect(tabs[0]!.getAttribute('aria-selected')).toBe('true');
    await fireEvent.click(tabs[1]!);
    expect(tabs[1]!.getAttribute('aria-selected')).toBe('true');
    expect(screen.getByRole('tabpanel').getAttribute('aria-labelledby')).toBe('tabs-tab-1');
  });

  it('goes from tab to tab with the arrow keys, round the end, down the side when on the left', async () => {
    const { unmount } = render(TabsBlock, { block, children });
    const tabs = screen.getAllByRole('tab');
    await fireEvent.keyDown(tabs[0]!, { key: 'ArrowLeft' });
    expect(tabs[2]!.getAttribute('aria-selected')).toBe('true');
    unmount();
    render(TabsBlock, { block: { ...block, side: 'left' }, children });
    expect(screen.getByRole('tablist').getAttribute('aria-orientation')).toBe('vertical');
    const side = screen.getAllByRole('tab');
    await fireEvent.keyDown(side[0]!, { key: 'ArrowDown' });
    expect(side[1]!.getAttribute('aria-selected')).toBe('true');
  });

  it('lights the name of a tab holding what the hovered button sends', () => {
    render(TabsBlock, { block, children, lit: [false, true, false] });
    expect(screen.getAllByRole('tab').map((t) => t.classList.contains('lit'))).toEqual([false, true, false]);
  });
});
