import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import Harness from './VideoOptions.harness.svelte';
import { memoryApi, setApiForTests, type MemoryOptions } from '../../lib/ipc';
import type { Block } from '../../lib/types';

type Video = Extract<Block, { type: 'video' }>;
const fresh = (patch: Partial<Video> = {}): Video => ({ id: 'clip', type: 'video', rect: { x: 0, y: 0, w: 8, h: 8 }, ...patch });

function open(block: Video, opts: MemoryOptions = {}) {
  setApiForTests(memoryApi(opts));
  const seen: { now: Video } = { now: block };
  render(Harness, { initial: block, onchange: (b: Video) => (seen.now = b) });
  return seen;
}

const select = (name: string) => screen.getByLabelText(name) as HTMLSelectElement;

afterEach(() => {
  cleanup();
  setApiForTests(undefined);
});

describe('VideoOptions', () => {
  it('starts on the whole screen with the computer\'s sound on and no microphone', async () => {
    open(fresh());
    await waitFor(() => expect(screen.getByRole('option', { name: 'Monitor of Built-in Audio Analog Stereo' })).toBeTruthy());
    expect(select('Record').value).toBe('screen');
    expect(select('Sound of the computer').value).toBe('default');
    expect(select('Microphone').value).toBe('off');
  });

  it('records the window in focus when asked, and says what happens when it cannot be found', async () => {
    const seen = open(fresh());
    await fireEvent.change(select('Record'), { target: { value: 'focused_window' } });
    expect(seen.now.subject).toBe('focused_window');
    expect(screen.getByText(/If it cannot be found, the whole screen is recorded/)).toBeTruthy();
    await fireEvent.change(select('Record'), { target: { value: 'screen' } });
    expect(seen.now.subject).toBe('screen');
  });

  it('lists the devices of the computer and stores the one picked by its name', async () => {
    const seen = open(fresh());
    await waitFor(() => expect(screen.getByRole('option', { name: 'Headset (Bluetooth)' })).toBeTruthy());
    await fireEvent.change(select('Microphone'), { target: { value: 'device:bluez_input.00_1B_66_00_00_00.0' } });
    expect(seen.now.audio).toEqual({ desktop: 'default', microphone: { device: 'bluez_input.00_1B_66_00_00_00.0' } });
    // the other sound is kept as it was
    await fireEvent.change(select('Sound of the computer'), { target: { value: 'off' } });
    expect(seen.now.audio).toEqual({ desktop: 'off', microphone: { device: 'bluez_input.00_1B_66_00_00_00.0' } });
    await fireEvent.change(select('Microphone'), { target: { value: 'default' } });
    expect(seen.now.audio?.microphone).toBe('default');
  });

  it('shows a device a layout names that this computer does not have, instead of changing it', async () => {
    const seen = open(fresh({ audio: { desktop: 'default', microphone: { device: 'Studio mic' } } }));
    await waitFor(() => expect(screen.getByRole('option', { name: 'Studio mic (not found on this computer)' })).toBeTruthy());
    expect(select('Microphone').value).toBe('device:Studio mic');
    // nothing was changed by looking
    expect(seen.now.audio).toEqual({ desktop: 'default', microphone: { device: 'Studio mic' } });
  });

  it('says why the computer\'s sound is missing on a Windows PC with no loopback device', async () => {
    open(fresh(), { noLoopback: true });
    await waitFor(() => expect(screen.getByRole('note').textContent).toContain('Stereo Mix'));
    expect(screen.queryByRole('option', { name: /Monitor of/ })).toBeNull();
    // the microphone is still there to pick
    expect(screen.getByRole('option', { name: 'Microphone (Realtek(R) Audio)' })).toBeTruthy();
  });

  it('says so when no sound system answers, and the choices still work', async () => {
    const seen = open(fresh(), { noAudio: true });
    await waitFor(() => expect(screen.getByRole('note').textContent).toContain('No sound system answered'));
    await fireEvent.change(select('Sound of the computer'), { target: { value: 'off' } });
    expect(seen.now.audio?.desktop).toBe('off');
  });
});
