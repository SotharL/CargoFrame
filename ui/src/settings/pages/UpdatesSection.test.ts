import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import UpdatesSection from './UpdatesSection.svelte';
import { memoryApi, setApiForTests, type Api, type MemoryOptions } from '../../lib/ipc';
import type { UpdateView } from '../../lib/types';

afterEach(() => {
  cleanup();
  setApiForTests(undefined);
});

function setup(update?: MemoryOptions['update'], over: Partial<Api> = {}) {
  const api = { ...memoryApi({ update }), ...over } as Api;
  setApiForTests(api);
  return api;
}

const check = () => screen.getByRole('button', { name: 'Check for updates' });

describe('Updates section', () => {
  it('shows the version and a Check for updates button', async () => {
    setup();
    render(UpdatesSection);
    expect(await screen.findByText('0.1.0-dev')).toBeTruthy();
    expect(check()).toBeTruthy();
  });

  it('says CargoFrame is up to date when nothing newer exists', async () => {
    setup();
    render(UpdatesSection);
    await fireEvent.click(await screen.findByRole('button', { name: 'Check for updates' }));
    expect(await screen.findByText('CargoFrame is up to date')).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Download and install' })).toBeNull();
  });

  it('offers a newer version with its notes, size and a Release page link', async () => {
    const api = setup({ newer: { version: '0.2.0', notes: '- Faster window\n- Fixes', size: 18 * 1024 * 1024 } });
    const open = vi.spyOn(api, 'openLink');
    render(UpdatesSection);
    await fireEvent.click(await screen.findByRole('button', { name: 'Check for updates' }));
    expect(await screen.findByText('Version 0.2.0 is available')).toBeTruthy();
    expect(screen.getByText(/CargoFrame-0\.2\.0-windows-x64-setup\.exe · 18\.0 MB/)).toBeTruthy();
    expect(screen.getByLabelText('Release notes').textContent).toContain('Faster window');
    await fireEvent.click(screen.getByRole('button', { name: 'Release page' }));
    expect(open).toHaveBeenCalledWith('https://github.com/SotharL/CargoFrame/releases/tag/v0.2.0');
  });

  it('starts the install only when the button is pressed, and says it is installing', async () => {
    const api = setup({ newer: { version: '0.2.0' } });
    const install = vi.spyOn(api, 'installUpdate');
    render(UpdatesSection);
    await fireEvent.click(await screen.findByRole('button', { name: 'Check for updates' }));
    expect(install).not.toHaveBeenCalled();
    await fireEvent.click(await screen.findByRole('button', { name: 'Download and install' }));
    expect(install).toHaveBeenCalledOnce();
    expect(await screen.findByText(/Installing\. CargoFrame will close/)).toBeTruthy();
    // While it works, the buttons cannot be pressed a second time.
    expect((check() as HTMLButtonElement).disabled).toBe(true);
    expect(screen.queryByRole('button', { name: 'Download and install' })).toBeNull();
  });

  it('shows download progress as it arrives from the app', async () => {
    let phase: UpdateView['phase'] = { state: 'downloading', done: 0, total: 20 * 1024 * 1024 };
    let ping: (() => void) | undefined;
    setup(undefined, {
      getUpdateView: async () => ({ ...(await memoryApi().getUpdateView()), phase }),
      onUpdateChanged: async (cb) => {
        ping = cb;
        return () => {};
      },
    });
    render(UpdatesSection);
    expect(await screen.findByText(/Downloading/)).toBeTruthy();
    expect(screen.getByText(/0 B of 20\.0 MB/)).toBeTruthy();
    // The section listens a moment after it first draws.
    await waitFor(() => expect(ping).toBeDefined());

    phase = { state: 'downloading', done: 5 * 1024 * 1024, total: 20 * 1024 * 1024 };
    ping!();
    await waitFor(() => expect(screen.getByText(/5\.0 MB of 20\.0 MB/)).toBeTruthy());
    const bar = screen.getByLabelText('Download progress') as HTMLProgressElement;
    expect(bar.value).toBe(5 * 1024 * 1024);
    expect(bar.max).toBe(20 * 1024 * 1024);

    phase = { state: 'verifying' };
    ping!();
    expect(await screen.findByText(/Checking the download against its checksum/)).toBeTruthy();
  });

  it('shows a failed install with where the checked installer is kept', async () => {
    setup(undefined, {
      getUpdateView: async () => ({
        ...(await memoryApi().getUpdateView()),
        phase: {
          state: 'failed',
          message: 'The installer did not take over. Run the downloaded installer yourself',
          installer: 'C:\\Users\\me\\AppData\\Local\\CargoFrame\\updates\\CargoFrame-0.2.0-windows-x64-setup.exe',
          needs_token: false,
        },
      }),
    });
    render(UpdatesSection);
    expect(await screen.findByText('The update did not finish')).toBeTruthy();
    expect(screen.getByRole('alert').textContent).toContain('did not take over');
    expect(screen.getByText(/CargoFrame-0\.2\.0-windows-x64-setup\.exe/)).toBeTruthy();
  });

  it('shows a failed check and points at the token when GitHub asked for one', async () => {
    setup({ fails: { message: 'GitHub says the repository was not found. It may be private.', needs_token: true } });
    render(UpdatesSection);
    await fireEvent.click(await screen.findByRole('button', { name: 'Check for updates' }));
    expect(await screen.findByText('Could not check')).toBeTruthy();
    expect(screen.getAllByRole('alert')[0]!.textContent).toContain('may be private');
    expect(document.querySelector('.token.ask')).toBeTruthy();
  });

  it('keeps the token write-only: stored shows a chip, never the value', async () => {
    const api = setup();
    const set = vi.spyOn(api, 'setUpdateToken');
    render(UpdatesSection);
    const input = await screen.findByLabelText('GitHub access token');
    await fireEvent.input(input, { target: { value: 'github_pat_secret' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Save GitHub access token' }));
    await waitFor(() => expect(set).toHaveBeenCalledWith('github_pat_secret'));
    expect(await screen.findByText(/Stored in your system's password store/)).toBeTruthy();
    expect(document.body.textContent).not.toContain('github_pat_secret');

    await fireEvent.click(screen.getByRole('button', { name: 'Remove' }));
    await waitFor(() => expect(set).toHaveBeenLastCalledWith(''));
    expect(await screen.findByLabelText('GitHub access token')).toBeTruthy();
  });

  it('says when the password store cannot be read', async () => {
    setup(undefined, {
      getUpdateView: async () => ({ ...(await memoryApi().getUpdateView()), token_problem: 'no secret service' }),
    });
    render(UpdatesSection);
    expect(await screen.findByText(/cannot be used: no secret service/)).toBeTruthy();
  });

  it('explains when there is no installer for this computer, and disables the check', async () => {
    setup(undefined, {
      getUpdateView: async () => ({ ...(await memoryApi().getUpdateView()), supported: false }),
    });
    render(UpdatesSection);
    expect(await screen.findByText(/no installer for this kind of computer/)).toBeTruthy();
    expect((check() as HTMLButtonElement).disabled).toBe(true);
  });

  it('tells the page when the daily check is switched', async () => {
    setup();
    const onauto = vi.fn();
    render(UpdatesSection, { auto: false, onauto });
    const toggle = await screen.findByRole('switch', { name: 'Look for a newer version once a day' });
    await fireEvent.click(toggle);
    expect(onauto).toHaveBeenCalledWith(true);
  });

  it('stops listening when it is closed', async () => {
    const stop = vi.fn();
    setup(undefined, { onUpdateChanged: async () => stop });
    const { unmount } = render(UpdatesSection);
    await screen.findByText('0.1.0-dev');
    unmount();
    // Closing before the listener is even registered still ends up stopping it.
    await waitFor(() => expect(stop).toHaveBeenCalledOnce());
  });
});
