import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import Video from './Video.svelte';
import { memoryApi, setApiForTests, type MemoryOptions } from '../../lib/ipc';

function open(opts: MemoryOptions = {}) {
  const api = memoryApi(opts);
  setApiForTests(api);
  render(Video);
  return api;
}

beforeEach(() => setApiForTests(undefined));
afterEach(() => {
  cleanup();
  setApiForTests(undefined);
});

describe('Video settings', () => {
  it('looks for ffmpeg when the page opens and says what it found and what Automatic would use', async () => {
    open();
    await screen.findByText('ffmpeg found');
    expect(screen.getByText('7.1-full_build')).toBeTruthy();
    expect(screen.getByText('GDI and Software')).toBeTruthy();
    expect(screen.getByText(/C:\\Tools\\ffmpeg\\bin\\ffmpeg.exe/)).toBeTruthy();
  });

  it('says plainly when ffmpeg is not there and does not offer a test recording', async () => {
    open({ noFfmpeg: true });
    await screen.findByText('ffmpeg not found');
    expect(screen.getByRole('alert').textContent).toContain('winget install Gyan.FFmpeg');
    expect((screen.getByRole('button', { name: 'Try a 5-second recording' }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('greys out what this ffmpeg does not have', async () => {
    open();
    await screen.findByText('ffmpeg found');
    const intel = screen.getByRole('option', { name: /Intel graphics.*not in this ffmpeg/ }) as HTMLOptionElement;
    expect(intel.disabled).toBe(true);
    const nvidia = screen.getByRole('option', { name: 'NVIDIA graphics card (NVENC)' }) as HTMLOptionElement;
    expect(nvidia.disabled).toBe(false);
    const x11 = screen.getByRole('option', { name: /X11.*not in this ffmpeg/ }) as HTMLOptionElement;
    expect(x11.disabled).toBe(true);
  });

  it('saves a change as soon as it is made', async () => {
    const api = open();
    await screen.findByText('ffmpeg found');
    await fireEvent.change(screen.getByRole('combobox', { name: 'Compression' }), { target: { value: 'nvenc' } });
    await screen.findByText('Saved');
    expect((await api.getVideo()).encoder).toBe('nvenc');
  });

  it('does not save a number that is not a number, and says so', async () => {
    const api = open();
    await screen.findByText('ffmpeg found');
    const fps = screen.getByRole('spinbutton', { name: 'Frames per second' });
    await fireEvent.input(fps, { target: { value: '' } });
    await fireEvent.change(fps);
    await screen.findByText('Frames per second must be a whole number');
    expect((await api.getVideo()).fps).toBe(30);
  });

  it('looks again when the place of ffmpeg changes', async () => {
    open({ noFfmpeg: true });
    await screen.findByText('ffmpeg not found');
    const where = screen.getByRole('textbox', { name: /Where ffmpeg is/ });
    await fireEvent.input(where, { target: { value: 'D:\\video' } });
    await fireEvent.change(where);
    await waitFor(() => expect(screen.getByRole('alert').textContent).toContain('was not found in D:\\video'));
  });

  it('records a test, shows the clip and opens it', async () => {
    open();
    await screen.findByText('ffmpeg found');
    await fireEvent.click(screen.getByRole('button', { name: 'Try a 5-second recording' }));
    await screen.findByText('It works');
    expect(screen.getByText('5.0 s, 1920×1080, 2.3 MB')).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: 'Open the clip' }));
    expect(screen.queryByText(/no test clip yet/)).toBeNull();
  });

  it('reports a recording that failed, with the advice that came with it', async () => {
    open({ recordingFails: true });
    await screen.findByText('ffmpeg found');
    await fireEvent.click(screen.getByRole('button', { name: 'Try a 5-second recording' }));
    await screen.findByText('It did not work');
    expect(screen.getByRole('alert').textContent).toContain('choose Software to carry on');
    expect(screen.queryByRole('button', { name: 'Open the clip' })).toBeNull();
  });
});
