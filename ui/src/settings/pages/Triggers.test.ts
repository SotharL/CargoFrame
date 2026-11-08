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


describe('Triggers on a Wayland desktop', () => {
  it('says how shortcuts work there, and gives the command to add by hand', async () => {
    await api.saveItem('trigger', 'bug', trigger({ name: 'Bug report', shortcut: 'Ctrl+Shift+F9', layout_id: 'pic' }));
    api.getShortcutStatus = async () => ({
      route: 'manual',
      note: 'This Wayland desktop does not let programs set shortcuts.',
      errors: {},
      commands: { bug: '/opt/CargoFrame.AppImage open --trigger bug' },
    });
    const written: string[] = [];
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText: async (t: string) => void written.push(t) },
    });
    render(Triggers);
    expect(await screen.findByRole('note')).toHaveProperty('textContent', 'This Wayland desktop does not let programs set shortcuts.');
    expect(await screen.findByText('/opt/CargoFrame.AppImage open --trigger bug')).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: 'Copy' }));
    await waitFor(() => expect(written).toEqual(['/opt/CargoFrame.AppImage open --trigger bug']));
    expect(await screen.findByRole('button', { name: 'Copied' })).toBeTruthy();
  });

  it('where the desktop took the shortcut, no command is shown unless it was refused', async () => {
    await api.saveItem('trigger', 'bug', trigger({ name: 'Bug report', shortcut: 'Ctrl+Shift+F9' }));
    await api.saveItem('trigger', 'chat', trigger({ name: 'Chat', shortcut: 'Ctrl+Shift+F10' }));
    api.getShortcutStatus = async () => ({
      route: 'gnome',
      note: 'GNOME',
      errors: { chat: 'GNOME did not take Ctrl+Shift+F10' },
      commands: { bug: 'cf open --trigger bug', chat: 'cf open --trigger chat' },
    });
    render(Triggers);
    expect(await screen.findByText('GNOME did not take Ctrl+Shift+F10')).toBeTruthy();
    expect(screen.queryByText('cf open --trigger bug')).toBeNull();
    expect(screen.getByText('cf open --trigger chat')).toBeTruthy();
  });
});
