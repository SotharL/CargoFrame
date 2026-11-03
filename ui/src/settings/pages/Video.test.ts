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
        expect(drawn).toContain('arc:960,540,108,0,' + Math.PI * 2);
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

    describe('editing on the picture', () => {
      const real = Element.prototype.getBoundingClientRect;
      beforeEach(() => {
        URL.createObjectURL = () => 'blob:stage';
        URL.revokeObjectURL = () => {};
        // the clip is shown at half its size: 960 px for 1920
        Element.prototype.getBoundingClientRect = function () {
          if (this.classList.contains('stage')) {
            return { width: 960, height: 540, left: 0, top: 0, right: 960, bottom: 540, x: 0, y: 0, toJSON() {} } as DOMRect;
          }
          // the timeline is 500 px wide: with a 5 s clip, 100 px is a second
          if (this.classList.contains('timeline')) {
            return { width: 500, height: 22, left: 0, top: 0, right: 500, bottom: 22, x: 0, y: 0, toJSON() {} } as DOMRect;
          }
          return real.call(this);
        };
      });
      afterEach(() => {
        Element.prototype.getBoundingClientRect = real;
      });

      async function watching() {
        open();
        await screen.findByText('ffmpeg found');
        await fireEvent.click(screen.getByRole('button', { name: 'Try a 5-second recording' }));
        await screen.findByText('It works');
        await fireEvent.click(screen.getByRole('button', { name: 'Watch it here' }));
        await waitFor(() => expect(document.querySelector('video')).not.toBeNull());
      }

      const value = (name: string) => (screen.getByRole('spinbutton', { name }) as HTMLInputElement).value;

      it('takes the start and the end from where the clip is playing', async () => {
        await watching();
        const video = document.querySelector('video')!;
        Object.defineProperty(video, 'currentTime', { value: 1.26, configurable: true });
        await fireEvent.click(screen.getByRole('button', { name: 'Start here' }));
        expect(value('Start (seconds)')).toBe('1.3');
        Object.defineProperty(video, 'currentTime', { value: 3.9, configurable: true });
        await fireEvent.click(screen.getByRole('button', { name: 'End here' }));
        expect(value('End (seconds)')).toBe('3.9');
      });

      it('shows the crop box on the picture only while cropping and moves it with the mouse', async () => {
        await watching();
        expect(screen.queryByLabelText('Crop area')).toBeNull();
        await fireEvent.click(screen.getByRole('checkbox', { name: 'Crop to an area' }));
        const box = screen.getByLabelText('Crop area');
        expect(box.style.left).toBe('25%');
        expect(box.style.width).toBe('50%');
        await fireEvent.mouseDown(box, { clientX: 100, clientY: 100 });
        await fireEvent.mouseMove(window, { clientX: 150, clientY: 120 });
        // 50 and 20 screen pixels at half size are 100 and 40 clip pixels
        expect(value('Left (px)')).toBe('580');
        expect(value('Top (px)')).toBe('310');
        // it cannot leave the picture
        await fireEvent.mouseMove(window, { clientX: 5000, clientY: 5000 });
        expect(value('Left (px)')).toBe('960');
        expect(value('Top (px)')).toBe('540');
        await fireEvent.mouseUp(window);
        await fireEvent.mouseMove(window, { clientX: 0, clientY: 0 });
        expect(value('Left (px)')).toBe('960');
      });

      it('resizes the crop box from its corner, on even numbers, and not past the edge', async () => {
        await watching();
        await fireEvent.click(screen.getByRole('checkbox', { name: 'Crop to an area' }));
        const grip = screen.getByLabelText('Resize the crop area');
        await fireEvent.mouseDown(grip, { clientX: 500, clientY: 300 });
        await fireEvent.mouseMove(window, { clientX: 531, clientY: 311 });
        expect(value('Width (px)')).toBe('1022');
        expect(value('Height (px)')).toBe('562');
        await fireEvent.mouseMove(window, { clientX: 5000, clientY: 5000 });
        // from x 480 and y 270
        expect(value('Width (px)')).toBe('1440');
        expect(value('Height (px)')).toBe('810');
        await fireEvent.mouseMove(window, { clientX: -5000, clientY: -5000 });
        expect(value('Width (px)')).toBe('16');
        await fireEvent.mouseUp(window);
      });

      function mockCanvas(): string[] {
        const drawn: string[] = [];
        const ctx = new Proxy({}, {
          get: (_t, name) => (typeof name === 'string' && ['arc', 'beginPath', 'stroke', 'strokeText', 'fillText'].includes(name)
            ? (...a: unknown[]) => { drawn.push(`${name}:${a.join(',')}`); }
            : undefined),
          set: () => true,
        });
        HTMLCanvasElement.prototype.getContext = (() => ctx) as never;
        HTMLCanvasElement.prototype.toBlob = function (cb: BlobCallback) {
          cb(new Blob([new Uint8Array(8)], { type: 'image/png' }));
        };
        return drawn;
      }

      it('puts the drawn mark where you click on the picture and writes your text under it', async () => {
        const drawn = mockCanvas();
        await watching();
        await fireEvent.change(screen.getByRole('combobox', { name: 'Mark' }), { target: { value: 'drawn' } });
        // the mark starts in the middle; the dot shows where it will go
        expect(screen.getByLabelText('Mark position').style.left).toBe('50%');
        // the picture is shown at half size: a click at 240,135 is at 480,270 in the clip
        await fireEvent.click(document.querySelector('.stage')!, { clientX: 240, clientY: 135 });
        expect(screen.getByLabelText('Mark position').style.left).toBe('25%');
        expect(screen.getByLabelText('Mark position').style.top).toBe('25%');
        await fireEvent.input(screen.getByRole('textbox', { name: 'Mark text' }), { target: { value: 'Look here' } });
        await fireEvent.click(screen.getByRole('button', { name: 'Export a copy' }));
        await screen.findByText('Copy made');
        expect(drawn).toContain('arc:480,270,108,0,' + Math.PI * 2);
        expect(drawn.some((d) => d.startsWith('fillText:Look here,480,'))).toBe(true);
      });

      it('draws only the circle when the text is empty, and ignores clicks unless a drawn mark is chosen or while cropping', async () => {
        const drawn = mockCanvas();
        await watching();
        // no drawn mark chosen: a click moves nothing and there is no dot
        await fireEvent.click(document.querySelector('.stage')!, { clientX: 240, clientY: 135 });
        expect(screen.queryByLabelText('Mark position')).toBeNull();
        await fireEvent.change(screen.getByRole('combobox', { name: 'Mark' }), { target: { value: 'drawn' } });
        await fireEvent.click(screen.getByRole('checkbox', { name: 'Crop to an area' }));
        await fireEvent.click(document.querySelector('.stage')!, { clientX: 240, clientY: 135 });
        await fireEvent.click(screen.getByRole('checkbox', { name: 'Crop to an area' }));
        expect(screen.getByLabelText('Mark position').style.left).toBe('50%');
        await fireEvent.input(screen.getByRole('textbox', { name: 'Mark text' }), { target: { value: '  ' } });
        await fireEvent.click(screen.getByRole('button', { name: 'Export a copy' }));
        await screen.findByText('Copy made');
        expect(drawn.some((d) => d.startsWith('arc:'))).toBe(true);
        expect(drawn.some((d) => d.startsWith('fillText'))).toBe(false);
      });

      const edge = (name: string) => screen.getByRole('slider', { name });

      it('shows the kept part on a timeline, and drags its start and end with the mouse', async () => {
        await watching();
        const timeline = document.querySelector('.timeline') as HTMLElement;
        const kept = timeline.querySelector('.kept') as HTMLElement;
        expect(kept.style.left).toBe('0%');
        expect(kept.style.width).toBe('100%');
        expect(edge('Start of the copy').getAttribute('aria-valuenow')).toBe('0');
        expect(edge('End of the copy').getAttribute('aria-valuenow')).toBe('5');

        await fireEvent.mouseDown(edge('Start of the copy'), { clientX: 0 });
        await fireEvent.mouseMove(window, { clientX: 130 });
        expect(value('Start (seconds)')).toBe('1.3');
        expect(kept.style.left).toBe('26%');
        // it cannot pass the end: a tenth of a second is always left
        await fireEvent.mouseMove(window, { clientX: 5000 });
        expect(value('Start (seconds)')).toBe('4.9');
        await fireEvent.mouseUp(window);
        await fireEvent.mouseMove(window, { clientX: 0 });
        expect(value('Start (seconds)')).toBe('4.9');

        await fireEvent.mouseDown(edge('End of the copy'), { clientX: 500 });
        await fireEvent.mouseMove(window, { clientX: 0 });
        expect(value('End (seconds)')).toBe('5');
        // the start is at 4.9 s, so the end cannot come back below 5.0 s
        await fireEvent.mouseMove(window, { clientX: 450 });
        expect(value('End (seconds)')).toBe('5');
        await fireEvent.mouseUp(window);
        await fireEvent.keyDown(edge('Start of the copy'), { key: 'ArrowLeft', shiftKey: true });
        expect(value('Start (seconds)')).toBe('3.9');
        await fireEvent.mouseDown(edge('End of the copy'), { clientX: 500 });
        await fireEvent.mouseMove(window, { clientX: 450 });
        expect(value('End (seconds)')).toBe('4.5');
        await fireEvent.mouseUp(window);
      });

      it('moves the ends from the keyboard, by a tenth of a second or a second with Shift', async () => {
        await watching();
        const start = edge('Start of the copy');
        await fireEvent.keyDown(start, { key: 'ArrowRight' });
        expect(value('Start (seconds)')).toBe('0.1');
        await fireEvent.keyDown(start, { key: 'ArrowUp', shiftKey: true });
        expect(value('Start (seconds)')).toBe('1.1');
        await fireEvent.keyDown(start, { key: 'ArrowLeft' });
        expect(value('Start (seconds)')).toBe('1');
        await fireEvent.keyDown(start, { key: 'a' });
        expect(value('Start (seconds)')).toBe('1');
        const end = edge('End of the copy');
        await fireEvent.keyDown(end, { key: 'ArrowRight' });
        expect(value('End (seconds)')).toBe('5');
        await fireEvent.keyDown(end, { key: 'ArrowLeft', shiftKey: true });
        expect(value('End (seconds)')).toBe('4');
        expect(end.getAttribute('aria-valuetext')).toBe('4.0 s');
      });

      it('takes the clip to the moment you click on the timeline, but not when you grab an end', async () => {
        await watching();
        const video = document.querySelector('video')!;
        const seen: number[] = [];
        Object.defineProperty(video, 'currentTime', { set: (v: number) => seen.push(v), get: () => 0, configurable: true });
        await fireEvent.click(document.querySelector('.timeline')!, { clientX: 250 });
        expect(seen).toEqual([2.5]);
        expect((document.querySelector('.playhead') as HTMLElement).style.left).toBe('50%');
        await fireEvent.click(edge('Start of the copy'), { clientX: 10 });
        expect(seen).toEqual([2.5]);
      });

      it('follows the playing clip with a playhead, and starts again from the whole clip for a new test', async () => {
        await watching();
        const video = document.querySelector('video')!;
        Object.defineProperty(video, 'currentTime', { value: 1, configurable: true });
        await fireEvent.timeUpdate(video);
        expect((document.querySelector('.playhead') as HTMLElement).style.left).toBe('20%');
        await fireEvent.keyDown(edge('Start of the copy'), { key: 'ArrowRight', shiftKey: true });
        expect(value('Start (seconds)')).toBe('1');
      });

      it('has no crop box or start and end buttons on the edited copy', async () => {
        await watching();
        await fireEvent.click(screen.getByRole('checkbox', { name: 'Crop to an area' }));
        await fireEvent.click(screen.getByRole('button', { name: 'Export a copy' }));
        await screen.findByText('Copy made');
        await fireEvent.click(screen.getByRole('button', { name: 'Watch the copy here' }));
        await waitFor(() => expect(document.querySelector('video')!.getAttribute('aria-label')).toBe('Edited copy'));
        expect(screen.queryByLabelText('Crop area')).toBeNull();
        expect(screen.queryByRole('button', { name: 'Start here' })).toBeNull();
        expect(document.querySelector('.timeline')).toBeNull();
      });
    });
  });
});
