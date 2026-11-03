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

  it('records until stopped, shows how it is going, then makes the clip', async () => {
    open({ video: { window_secs: 3 } });
    await screen.findByText('ffmpeg found');
    await fireEvent.click(screen.getByRole('button', { name: 'Record until I stop' }));
    await screen.findByText('Recording', { selector: '.chip' });
    expect(screen.getByText(/The last \d+ s are kept \(up to 3 s\)/)).toBeTruthy();
    // while it runs the 5-second test is not offered and the other button turns into Stop
    expect((screen.getByRole('button', { name: 'Try a 5-second recording' }) as HTMLButtonElement).disabled).toBe(true);
    await fireEvent.click(screen.getByRole('button', { name: 'Stop and make the clip' }));
    await screen.findByText('It works');
    expect(screen.queryByText('Recording', { selector: '.chip' })).toBeNull();
    expect(screen.getByRole('button', { name: 'Record until I stop' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Open the clip' })).toBeTruthy();
  });

  it('tells why a recording ended by itself, and goes back to the start', async () => {
    open({ recordingFails: true });
    await screen.findByText('ffmpeg found');
    await fireEvent.click(screen.getByRole('button', { name: 'Record until I stop' }));
    await screen.findByText('It did not work', undefined, { timeout: 3000 });
    expect(screen.getByRole('alert').textContent).toContain('choose Software to carry on');
    expect(screen.getByRole('button', { name: 'Record until I stop' })).toBeTruthy();
  });

  it('picks up a recording that is already running when the page opens', async () => {
    const api = memoryApi();
    setApiForTests(api);
    await api.startVideoRecording({ ffmpeg: null, capture: 'auto', encoder: 'auto', fps: 30, window_secs: 10, max_height: 1080 });
    render(Video);
    await screen.findByText('Recording', { selector: '.chip' });
    expect(screen.getByRole('button', { name: 'Stop and make the clip' })).toBeTruthy();
    cleanup();
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

  describe('trimming, cropping and marking a copy', () => {
    async function recordATest() {
      open();
      await screen.findByText('ffmpeg found');
      await fireEvent.click(screen.getByRole('button', { name: 'Try a 5-second recording' }));
      await screen.findByText('It works');
    }

    it('starts from the whole clip and makes a copy of the cut', async () => {
      await recordATest();
      expect((screen.getByRole('spinbutton', { name: 'Start (seconds)' }) as HTMLInputElement).value).toBe('0');
      expect((screen.getByRole('spinbutton', { name: 'End (seconds)' }) as HTMLInputElement).value).toBe('5');
      await fireEvent.input(screen.getByRole('spinbutton', { name: 'Start (seconds)' }), { target: { value: '1' } });
      await fireEvent.input(screen.getByRole('spinbutton', { name: 'End (seconds)' }), { target: { value: '3.5' } });
      await fireEvent.click(screen.getByRole('button', { name: 'Export a copy' }));
      await screen.findByText('Copy made');
      expect(screen.getByText(/Made a copy of 2\.5 s at 1920×1080/)).toBeTruthy();
      expect(screen.getByRole('button', { name: 'Open the copy' })).toBeTruthy();
    });

    it('offers a crop box in the middle of the picture and crops to even sizes', async () => {
      await recordATest();
      expect(screen.queryByRole('spinbutton', { name: 'Width (px)' })).toBeNull();
      await fireEvent.click(screen.getByRole('checkbox', { name: 'Crop to an area' }));
      expect((screen.getByRole('spinbutton', { name: 'Left (px)' }) as HTMLInputElement).value).toBe('480');
      expect((screen.getByRole('spinbutton', { name: 'Width (px)' }) as HTMLInputElement).value).toBe('960');
      await fireEvent.input(screen.getByRole('spinbutton', { name: 'Width (px)' }), { target: { value: '641' } });
      await fireEvent.click(screen.getByRole('button', { name: 'Export a copy' }));
      await screen.findByText('Copy made');
      expect(screen.getByText(/at 640×540/)).toBeTruthy();
    });

    it('says in words why a cut cannot be made', async () => {
      await recordATest();
      await fireEvent.input(screen.getByRole('spinbutton', { name: 'End (seconds)' }), { target: { value: '0.2' } });
      await fireEvent.click(screen.getByRole('button', { name: 'Export a copy' }));
      await screen.findByText('No copy made');
      expect(screen.getAllByRole('alert').map((a) => a.textContent).join(' ')).toContain('must be at least 0.5 s');
      expect(screen.queryByRole('button', { name: 'Open the copy' })).toBeNull();
    });

    it('plays the copy in the window', async () => {
      URL.createObjectURL = () => 'blob:copy';
      URL.revokeObjectURL = () => {};
      await recordATest();
      await fireEvent.click(screen.getByRole('button', { name: 'Export a copy' }));
      await screen.findByText('Copy made');
      await fireEvent.click(screen.getByRole('button', { name: 'Watch the copy here' }));
      await waitFor(() => expect(document.querySelector('video')).not.toBeNull());
      expect(document.querySelector('video')!.getAttribute('aria-label')).toBe('Edited copy');
    });

    it('forgets the copy when a new test clip is recorded', async () => {
      await recordATest();
      await fireEvent.click(screen.getByRole('button', { name: 'Export a copy' }));
      await screen.findByText('Copy made');
      await fireEvent.click(screen.getByRole('button', { name: 'Try a 5-second recording' }));
      await waitFor(() => expect(screen.queryByText('Copy made')).toBeNull());
    });


    describe('a mark the window draws', () => {
      const drawn: string[] = [];
      let canvasWorks = true;
      beforeEach(() => {
        drawn.length = 0;
        canvasWorks = true;
        const ctx = new Proxy({}, {
          get: (_t, name) => (typeof name === 'string' && ['arc', 'beginPath', 'stroke', 'strokeText', 'fillText'].includes(name)
            ? (...a: unknown[]) => { drawn.push(`${name}:${a.join(',')}`); }
            : undefined),
          set: () => true,
        });
        HTMLCanvasElement.prototype.getContext = (() => (canvasWorks ? ctx : null)) as never;
        HTMLCanvasElement.prototype.toBlob = function (cb: BlobCallback) {
          cb(new Blob([new Uint8Array(8)], { type: 'image/png' }));
        };
      });

      it('draws a circle and a text the size of the clip, hands them over, then exports', async () => {
        const api = memoryApi();
        const sent: (Blob | null)[] = [];
        const original = api.setVideoTestMark;
        api.setVideoTestMark = async (png) => {
          sent.push(png);
          return original(png);
        };
        setApiForTests(api);
        render(Video);
        await screen.findByText('ffmpeg found');
        await fireEvent.click(screen.getByRole('button', { name: 'Try a 5-second recording' }));
        await screen.findByText('It works');
        await fireEvent.change(screen.getByRole('combobox', { name: 'Mark' }), { target: { value: 'drawn' } });
        await fireEvent.click(screen.getByRole('button', { name: 'Export a copy' }));
        await screen.findByText('Copy made');
        expect(sent).toHaveLength(1);
        expect(sent[0]!.type).toBe('image/png');
        // a circle in the middle of the 1920x1080 picture, and the text
        expect(drawn).toContain('arc:960,540,216,0,' + Math.PI * 2);
        expect(drawn.some((d) => d.startsWith('fillText:CargoFrame test'))).toBe(true);
      });

      it('says so when the window cannot draw', async () => {
        canvasWorks = false;
        open();
        await screen.findByText('ffmpeg found');
        await fireEvent.click(screen.getByRole('button', { name: 'Try a 5-second recording' }));
        await screen.findByText('It works');
        await fireEvent.change(screen.getByRole('combobox', { name: 'Mark' }), { target: { value: 'drawn' } });
        await fireEvent.click(screen.getByRole('button', { name: 'Export a copy' }));
        await screen.findByText('No copy made');
        expect(screen.getAllByRole('alert').map((a) => a.textContent).join(' ')).toContain('cannot draw the mark');
      });
    });
  });
});
