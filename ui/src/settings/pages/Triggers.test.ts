import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import Triggers from './Triggers.svelte';
import { memoryApi, setApiForTests, type Api } from '../../lib/ipc';
import type { Layout, Trigger } from '../../lib/types';

let api: Api;

const shotLayout: Layout = { id: '', name: 'Picture', blocks: [{ id: 'shot', type: 'screenshot', rect: { x: 0, y: 0, w: 8, h: 8 } }] };
const clipLayout: Layout = { id: '', name: 'Clip', blocks: [{ id: 'clip', type: 'video', rect: { x: 0, y: 0, w: 8, h: 8 } }] };
const trigger = (over: Partial<Trigger>): Trigger => ({
  name: 'T', enabled: true, shortcut: 'F9', capture: 'active_window', media: 'screenshot', layout_id: null, ...over,
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

describe('Triggers: screenshot or video', () => {
  it('a new trigger takes a screenshot, and choosing a video hides the choice of what to capture', async () => {
    await api.saveItem('trigger', 'first', trigger({ name: 'First' }));
    render(Triggers);
    await fireEvent.click(await screen.findByRole('button', { name: 'Add a trigger' }));
    const makes = screen.getByRole('combobox', { name: 'Makes' }) as HTMLSelectElement;
    expect(makes.value).toBe('screenshot');
    expect(screen.getByRole('combobox', { name: 'Capture' })).toBeTruthy();
    await fireEvent.change(makes, { target: { value: 'video' } });
    expect(screen.queryByRole('combobox', { name: 'Capture' })).toBeNull();
    expect(screen.getByText(/The first press starts recording the screen/)).toBeTruthy();
  });

  it('saves the choice, and the card says it records a video', async () => {
    await api.saveItem('trigger', 'first', trigger({ name: 'First' }));
    render(Triggers);
    await fireEvent.click(await screen.findByRole('button', { name: 'Add a trigger' }));
    await fireEvent.input(screen.getByPlaceholderText('Bug report'), { target: { value: 'Clip it' } });
    await fireEvent.change(screen.getByRole('combobox', { name: 'Makes' }), { target: { value: 'video' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await screen.findByText('Clip it');
    expect(await screen.findByText(/Records a video of the screen/)).toBeTruthy();
    const saved = (await api.listItems<Trigger>('trigger')).items.find((i) => i.item.name === 'Clip it');
    expect(saved?.item.media).toBe('video');
  });

  it('warns when the layout does not match what the trigger makes, and only then', async () => {
    await api.saveItem('trigger', 'a', trigger({ name: 'Video, picture layout', media: 'video', layout_id: 'pic' }));
    await api.saveItem('trigger', 'b', trigger({ name: 'Picture, clip layout', shortcut: 'F10', layout_id: 'clip' }));
    await api.saveItem('trigger', 'c', trigger({ name: 'Video, clip layout', shortcut: 'F11', media: 'video', layout_id: 'clip' }));
    await api.saveItem('trigger', 'd', trigger({ name: 'Picture, picture layout', shortcut: 'F12', layout_id: 'pic' }));
    render(Triggers);
    await screen.findByText('Video, picture layout');
    await waitFor(() => expect(screen.getAllByText(/Video block|clip would not show/).length).toBe(2));
    expect(screen.getByText('This layout has no Video block, so the clip would not show')).toBeTruthy();
    expect(screen.getByText('This layout has a Video block but this trigger takes a screenshot')).toBeTruthy();
  });
});
