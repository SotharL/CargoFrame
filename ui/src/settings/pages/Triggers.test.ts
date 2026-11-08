import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import Triggers from './Triggers.svelte';
import { memoryApi, setApiForTests, type Api } from '../../lib/ipc';
import type { Layout, Trigger } from '../../lib/types';

let api: Api;

const shotLayout: Layout = { id: '', name: 'Picture', blocks: [{ id: 'shot', type: 'screenshot', rect: { x: 0, y: 0, w: 8, h: 8 } }] };
const clipLayout: Layout = { id: '', name: 'Clip', blocks: [{ id: 'clip', type: 'video', rect: { x: 0, y: 0, w: 8, h: 8 } }] };
const trigger = (over: Partial<Trigger>): Trigger => ({
  name: 'T', enabled: true, shortcut: 'F9', capture: 'active_window', layout_id: null, ...over,
});

beforeEach(async () => {
  api = memoryApi();
  await api.saveItem('layout', 'pic', shotLayout);
  await api.saveItem('layout', 'clip', clipLayout);
  setApiForTests(api);
});
afterEach(() => {
  cleanup();
  setApiForTests(undefined);
});

describe('Triggers: the layout says screenshot or recording', () => {
  it('a layout without a Video block takes a screenshot, and asks of what', async () => {
    render(Triggers);
    await fireEvent.click(await screen.findByRole('button', { name: 'Add a trigger' }));
    expect(screen.queryByRole('combobox', { name: 'Makes' })).toBeNull();
    await fireEvent.change(screen.getByRole('combobox', { name: 'Layout' }), { target: { value: 'pic' } });
    expect(screen.getByRole('combobox', { name: 'Screenshot of' })).toBeTruthy();
    expect(screen.getByText(/A press takes a screenshot/)).toBeTruthy();
  });

  it('a layout with a Video block records, and the screenshot choice goes away', async () => {
    render(Triggers);
    await fireEvent.click(await screen.findByRole('button', { name: 'Add a trigger' }));
    await fireEvent.change(screen.getByRole('combobox', { name: 'Layout' }), { target: { value: 'clip' } });
    expect(screen.queryByRole('combobox', { name: 'Screenshot of' })).toBeNull();
    expect(screen.getByText(/a press records the screen/)).toBeTruthy();
  });

  it('each card says what a press does, from its layout, whatever an old file says', async () => {
    // an old trigger file that still says "video" for a picture layout, and the other way round
    await api.saveItem('trigger', 'a', { ...trigger({ name: 'Old video, picture layout', layout_id: 'pic' }), media: 'video' });
    await api.saveItem('trigger', 'b', trigger({ name: 'Clip layout', shortcut: 'F10', layout_id: 'clip' }));
    render(Triggers);
    await screen.findByText('Old video, picture layout');
    await waitFor(() => expect(screen.getAllByText(/Records the screen/).length).toBe(1));
    expect(screen.getByText(/Captures the program in focus/)).toBeTruthy();
  });
});

