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
  it('shows a red outline and a blinking dot while recording, in the corner chosen, and saves the choice', async () => {
    const api = open();
    await screen.findByText('ffmpeg found');
    const outline = screen.getByRole('checkbox', { name: /red outline/ }) as HTMLInputElement;
    const dot = screen.getByRole('checkbox', { name: /red dot/ }) as HTMLInputElement;
    expect(outline.checked && dot.checked).toBe(true);
    await fireEvent.change(screen.getByRole('combobox', { name: 'Dot in the corner' }), { target: { value: 'bottom_left' } });
    await fireEvent.click(outline);
    await waitFor(async () => expect((await api.getVideo()).sign).toEqual({ outline: false, dot: true, corner: 'bottom_left' }));
    await fireEvent.click(dot);
    expect(screen.queryByRole('combobox', { name: 'Dot in the corner' })).toBeNull();
  });

  it('turns on continuous recording, saved, and says how it works', async () => {
    const api = open();
    await screen.findByText('ffmpeg found');
    const box = screen.getByRole('checkbox', { name: /record continuously/ }) as HTMLInputElement;
    expect(box.checked).toBe(false);
    expect(screen.queryByText(/acts when it is let go/)).toBeNull();
    await fireEvent.click(box);
    await waitFor(async () => expect((await api.getVideo()).continuous).toBe(true));
    expect(screen.getByText(/acts when it is let go/)).toBeTruthy();
  });

  it('follows a moved window only when asked, and says what it costs', async () => {
    const api = open();
    await screen.findByText('ffmpeg found');
    const box = screen.getByRole('checkbox', { name: 'Follow the window when it is moved' }) as HTMLInputElement;
    expect(box.checked).toBe(false);
    expect(screen.getByText(/keeps the place and size the window had/)).toBeTruthy();
    await fireEvent.click(box);
    await waitFor(async () => expect((await api.getVideo()).follow_window).toBe(true));
    expect(screen.getByText(/uses more of the computer than one window/)).toBeTruthy();
  });

  it('looks for ffmpeg when the page opens and says what it found and what Automatic would use', async () => {
    open();
    await screen.findByText('ffmpeg found');
    expect(screen.getByText('7.1-full_build')).toBeTruthy();
    expect(screen.getByText('GDI and Software')).toBeTruthy();
    expect(screen.getByText(/C:\\Tools\\ffmpeg\\bin\\ffmpeg.exe/)).toBeTruthy();
  });

  it('sends the person to Setup to install ffmpeg when it is not there', async () => {
    open({ noFfmpeg: true });
    await screen.findByText('ffmpeg not found');
    expect(screen.getByRole('button', { name: 'Install it from Setup' })).toBeTruthy();
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

  describe('watching the clip in the window', () => {
    const made: string[] = [];
    const revoked: string[] = [];
    beforeEach(() => {
      made.length = 0;
      revoked.length = 0;
      URL.createObjectURL = () => {
        made.push(`blob:clip-${made.length}`);
        return made[made.length - 1]!;
      };
      URL.revokeObjectURL = (u: string) => {
        revoked.push(u);
      };
    });

    async function recordATest() {
      open();
      await screen.findByText('ffmpeg found');
      await fireEvent.click(screen.getByRole('button', { name: 'Try a 5-second recording' }));
      await screen.findByText('It works');
    }

    it('plays the clip from a blob the page made, and lets go of it afterwards', async () => {
      await recordATest();
      expect(document.querySelector('video')).toBeNull();
      await fireEvent.click(screen.getByRole('button', { name: 'Watch it here' }));
      await waitFor(() => expect(document.querySelector('video')).not.toBeNull());
      expect(document.querySelector('video')!.getAttribute('src')).toBe('blob:clip-0');
      // a new recording takes the old player away
      await fireEvent.click(screen.getByRole('button', { name: 'Try a 5-second recording' }));
      await screen.findByText('It works');
      expect(document.querySelector('video')).toBeNull();
      expect(revoked).toEqual(['blob:clip-0']);
    });

    it('says so, and points to the player, when the window cannot play the clip', async () => {
      await recordATest();
      await fireEvent.click(screen.getByRole('button', { name: 'Watch it here' }));
      await waitFor(() => expect(document.querySelector('video')).not.toBeNull());
      await fireEvent.error(document.querySelector('video')!);
      await waitFor(() => expect(document.querySelector('video')).toBeNull());
      expect(screen.getByRole('alert').textContent).toContain('cannot play the clip');
      expect(screen.getByRole('alert').textContent).toContain('video player');
      expect(revoked).toEqual(['blob:clip-0']);
    });
  });
});
