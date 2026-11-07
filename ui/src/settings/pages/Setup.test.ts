import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import Setup from './Setup.svelte';
import { setupState } from '../setupState.svelte';
import { memoryApi, setApiForTests, type MemoryOptions } from '../../lib/ipc';

function open(opts: MemoryOptions = {}) {
  setApiForTests(memoryApi(opts));
  return render(Setup, { pollMs: 10, minCheckMs: 0 });
}

beforeEach(() => {
  setupState.missing = 0;
  setApiForTests(undefined);
});
afterEach(() => {
  cleanup();
  setApiForTests(undefined);
});

describe('Setup', () => {
  it('says plainly that nothing is missing and offers no install', async () => {
    open({ setup: 'ready' });
    await screen.findByText('Everything CargoFrame can use is here');
    expect(screen.queryByRole('button', { name: 'Install ffmpeg' })).toBeNull();
    expect(screen.getAllByText('Found').length).toBe(3);
    expect(setupState.missing).toBe(0);
  });

  it('on Windows with ffmpeg missing there is one button, and it installs and then finds ffmpeg', async () => {
    open({ setup: 'windows_missing', installPolls: 2 });
    await screen.findByText('1 app is missing');
    expect(setupState.missing).toBe(1);
    expect(screen.getByText(/no administrator password/)).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: 'Install ffmpeg' }));
    await screen.findByText('Installing ffmpeg…');
    // the page asks again by itself until the install is over
    await screen.findByText('Everything CargoFrame can use is here', {}, { timeout: 3000 });
    expect(screen.queryByText('Installing ffmpeg…')).toBeNull();
    expect(setupState.missing).toBe(0);
  });

  it('shows why an install failed and lets the person try again', async () => {
    open({ setup: 'windows_missing', installPolls: 1, installFails: true });
    await fireEvent.click(await screen.findByRole('button', { name: 'Install ffmpeg' }));
    const alert = await screen.findByRole('alert', {}, { timeout: 3000 });
    expect(alert.textContent).toContain('The install did not work');
    expect(alert.textContent).toContain('No network connection');
    expect(screen.getByRole('button', { name: 'Try again' })).toBeTruthy();
  });

  it('without winget it explains what to do instead of offering a button that cannot work', async () => {
    open({ setup: 'windows_no_winget' });
    await screen.findByText(/App Installer/);
    expect(screen.queryByRole('button', { name: 'Install ffmpeg' })).toBeNull();
  });

  it('names the default sound output, says videos record it, and opens the sound settings', async () => {
    open({ setup: 'windows_ready' });
    await screen.findByText(/Speakers \(Realtek\(R\) Audio\)\. Videos record this sound\./, { selector: 'p.detail' });
    expect(screen.queryByText(/is missing|are missing/)).toBeNull();
    await fireEvent.click(screen.getByRole('button', { name: 'Open the Windows sound settings' }));
    expect(screen.queryByRole('alert')).toBeNull();
  });

  it('raises attention when no sound output is set, and Check again sees one that was plugged in', async () => {
    open({ setup: 'windows_no_output', setupThen: 'ready' });
    await screen.findByText('1 app is missing');
    await screen.findByText(/no default playback device/, { selector: 'p.detail' });
    expect(setupState.missing).toBe(1);
    await fireEvent.click(screen.getByRole('button', { name: 'Check again' }));
    await screen.findByText('Everything CargoFrame can use is here');
    expect(screen.queryByText(/no default playback device/)).toBeNull();
    expect(setupState.missing).toBe(0);
  });

  it('on Linux the command is shown and can be copied, never run by the app', async () => {
    const written: string[] = [];
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText: async (t: string) => void written.push(t) },
    });
    open({ setup: 'linux_missing' });
    await screen.findByText('sudo apt install ffmpeg xdotool');
    await fireEvent.click(screen.getByRole('button', { name: 'Copy the command' }));
    await waitFor(() => expect(written).toEqual(['sudo apt install ffmpeg xdotool']));
    await screen.findByRole('button', { name: 'Copied' });
    expect(screen.queryByRole('button', { name: 'Install ffmpeg' })).toBeNull();
  });

  it('Check again looks at the computer again', async () => {
    open({ setup: 'windows_missing' });
    await screen.findByText('1 app is missing');
    await fireEvent.click(screen.getByRole('button', { name: 'Check again' }));
    await screen.findByText('1 app is missing');
  });
  it('lists the Claude desktop app when a Claude connection opens it, and the one button installs both', async () => {
    const api = memoryApi({ setup: 'windows_missing', claudeApp: 'missing', installPolls: 1 });
    await api.saveItem('connection', 'claude', {
      kind: 'claude', name: 'Claude', enabled: true, open_with: 'desktop', send_automatically: false,
    } as never);
    setApiForTests(api);
    render(Setup, { pollMs: 10, minCheckMs: 0 });
    await screen.findByText('2 apps are missing');
    expect(screen.getByText('Claude desktop app')).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: 'Install what is missing' }));
    await screen.findByText('Everything CargoFrame can use is here', {}, { timeout: 3000 });
    expect(setupState.missing).toBe(0);
  });

  it('is on the page at once, with a turning ring, and then shows a check for each line', async () => {
    setApiForTests(memoryApi({ setup: 'ready' }));
    render(Setup, { pollMs: 10, minCheckMs: 120 });
    // before the answer the place is already there: no jump when it arrives
    expect(screen.getByText('This computer')).toBeTruthy();
    expect(screen.getAllByRole('img', { name: /Checking/ }).length).toBeGreaterThan(0);
    expect(screen.queryByText('Found')).toBeNull();
    await screen.findByText('Everything CargoFrame can use is here');
    expect(screen.queryByRole('img', { name: /Checking/ })).toBeNull();
    expect(screen.getAllByText('Found').length).toBe(3);
  });

  it('keeps a line that is fine closed, and opens the one that needs attention', async () => {
    open({ setup: 'windows_missing' });
    await screen.findByText('1 app is missing');
    const open_ = [...document.querySelectorAll('details')].filter((d) => d.open);
    expect(open_).toHaveLength(1);
    expect(open_[0]!.textContent).toContain('Missing');
    cleanup();
    open({ setup: 'ready' });
    await screen.findByText('Everything CargoFrame can use is here');
    expect([...document.querySelectorAll('details')].some((d) => d.open)).toBe(false);
  });

  it('Check again turns every line back into a ring until the answer is in', async () => {
    setApiForTests(memoryApi({ setup: 'ready' }));
    render(Setup, { pollMs: 10, minCheckMs: 150 });
    await screen.findByText('Everything CargoFrame can use is here');
    await fireEvent.click(screen.getByRole('button', { name: 'Check again' }));
    expect(screen.getAllByRole('img', { name: /Checking/ }).length).toBeGreaterThan(1);
    expect(screen.queryByText('Found')).toBeNull();
    await screen.findByText('Everything CargoFrame can use is here');
    expect(screen.getAllByText('Found').length).toBe(3);
  });
});

