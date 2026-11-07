import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import VideoBlock from './VideoBlock.svelte';
import Harness from './VideoBlock.harness.svelte';
import type { ClipEdit } from '../../lib/types';
import { memoryApi, setApiForTests, type Api } from '../../lib/ipc';

const info = { width: 1280, height: 720, duration_ms: 9_500, bytes: 3 * 1024 * 1024, audio: true };
let revoked: string[];

beforeEach(() => {
  revoked = [];
  URL.createObjectURL = () => 'blob:clip';
  URL.revokeObjectURL = (u: string) => {
    revoked.push(u);
  };
});
afterEach(() => {
  cleanup();
  setApiForTests(undefined);
});

describe('VideoBlock', () => {
  it('plays the recording and says how long, how big and how large it is', async () => {
    setApiForTests(memoryApi());
    render(VideoBlock, { info });
    await waitFor(() => expect(document.querySelector('video')?.getAttribute('src')).toBe('blob:clip'));
    expect(screen.getByLabelText('Recording')).toBeTruthy();
    expect(screen.getByText('9.5 s · 1280×720 · 3.0 MB')).toBeTruthy();
  });

  it('lets go of the clip when the window moves on', async () => {
    setApiForTests(memoryApi());
    const { unmount } = render(VideoBlock, { info });
    await waitFor(() => expect(document.querySelector('video')).not.toBeNull());
    unmount();
    expect(revoked).toEqual(['blob:clip']);
  });

  it('says so when the window cannot play the recording, which can still be sent', async () => {
    setApiForTests(memoryApi());
    render(VideoBlock, { info });
    const video = await waitFor(() => {
      const v = document.querySelector('video');
      expect(v).not.toBeNull();
      return v!;
    });
    await fireEvent.error(video);
    expect(await screen.findByText(/cannot play the recording, but it is there and will be sent/)).toBeTruthy();
    expect(document.querySelector('video')).toBeNull();
    // the facts about it stay
    expect(screen.getByText('9.5 s · 1280×720 · 3.0 MB')).toBeTruthy();
  });

  it('shows why when the recording cannot be read', async () => {
    const api: Api = { ...memoryApi(), getSessionClip: async () => { throw 'The recording is no longer there: record it again'; } };
    setApiForTests(api);
    render(VideoBlock, { info });
    expect((await screen.findByRole('alert')).textContent).toContain('record it again');
    expect(document.querySelector('video')).toBeNull();
  });

  describe('cutting', () => {
    const slider = (name: string) => screen.getByRole('slider', { name });
    const startHandle = () => slider('Start of the part to keep');
    const endHandle = () => slider('End of the part to keep');

    async function ready() {
      setApiForTests(memoryApi());
      const seen: (ClipEdit | null)[] = [];
      render(Harness, { info, onedit: (t: ClipEdit | null) => seen.push(t) });
      const video = await waitFor(() => {
        const v = document.querySelector('video');
        expect(v).not.toBeNull();
        return v as HTMLVideoElement;
      });
      return { video, last: () => seen[seen.length - 1] };
    }
    /** What the player says the position is (jsdom plays nothing). */
    async function playingAt(video: HTMLVideoElement, secs: number, paused = true) {
      Object.defineProperty(video, 'currentTime', { value: secs, writable: true, configurable: true });
      Object.defineProperty(video, 'paused', { value: paused, configurable: true });
      await fireEvent.timeUpdate(video);
    }

    it('keeps all of the recording until a handle is moved, and says what is kept after', async () => {
      const { last } = await ready();
      expect(last()).toBeNull();
      expect((screen.getByRole('button', { name: 'Reset edits' }) as HTMLButtonElement).disabled).toBe(true);
      await fireEvent.keyDown(startHandle(), { key: 'ArrowRight', shiftKey: true });
      expect(last()).toEqual({ start_ms: 1000, end_ms: null, crop: null, mark: false });
      await fireEvent.keyDown(endHandle(), { key: 'ArrowLeft' });
      expect(last()).toEqual({ start_ms: 1000, end_ms: 9400, crop: null, mark: false });
      expect(screen.getByText('Keeping 1.0 s to 9.4 s (8.4 s of 9.5 s)')).toBeTruthy();
    });

    it('has no "Start here" or "End here": the handles on the timeline do the cutting', async () => {
      await ready();
      expect(screen.queryByRole('button', { name: 'Start here' })).toBeNull();
      expect(screen.queryByRole('button', { name: 'End here' })).toBeNull();
    });

    it('puts everything back with one button', async () => {
      const { last } = await ready();
      await fireEvent.keyDown(endHandle(), { key: 'ArrowLeft', shiftKey: true });
      expect(last()).not.toBeNull();
      await fireEvent.click(screen.getByRole('button', { name: 'Reset edits' }));
      expect(last()).toBeNull();
      expect(screen.getByText('9.5 s · 1280×720 · 3.0 MB')).toBeTruthy();
    });

    it('stops playing at the end of the part that is kept and starts again from its start', async () => {
      const { video } = await ready();
      video.pause = vi.fn();
      await fireEvent.keyDown(startHandle(), { key: 'ArrowRight', shiftKey: true });
      await fireEvent.keyDown(endHandle(), { key: 'ArrowLeft', shiftKey: true });
      await fireEvent.keyDown(endHandle(), { key: 'ArrowLeft', shiftKey: true });
      // kept: 1.0 s to 7.5 s
      await playingAt(video, 7.6, false);
      expect(video.pause).toHaveBeenCalled();
      expect(video.currentTime).toBe(7.5);
      await fireEvent.play(video);
      expect(video.currentTime).toBe(1);
    });

    it('can still be cut when this window cannot play the recording', async () => {
      const { video, last } = await ready();
      await fireEvent.error(video);
      await screen.findByText(/cannot play the recording/);
      await fireEvent.keyDown(startHandle(), { key: 'ArrowRight', shiftKey: true });
      expect(last()).toEqual({ start_ms: 1000, end_ms: null, crop: null, mark: false });
    });

    describe('crop', () => {
      it('starts from a box inside the picture that is already what is sent, and Apply keeps it', async () => {
        const { last } = await ready();
        await fireEvent.click(screen.getByRole('button', { name: 'Crop' }));
        expect(last()).toEqual({ start_ms: 0, end_ms: null, crop: { x: 128, y: 72, w: 1024, h: 576 }, mark: false });
        expect(screen.getByText('Keeping all 9.5 s, cropped to 1024×576')).toBeTruthy();
        await fireEvent.click(screen.getByRole('button', { name: 'Apply crop' }));
        expect(screen.queryByRole('group', { name: /Crop area/ })).toBeNull();
        expect(last()?.crop).toEqual({ x: 128, y: 72, w: 1024, h: 576 });
        expect(screen.getByRole('button', { name: 'Edit crop' })).toBeTruthy();
      });

      it('Cancel puts back what was there before, and Reset goes back to the full capture', async () => {
        const { last } = await ready();
        await fireEvent.click(screen.getByRole('button', { name: 'Crop' }));
        await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
        expect(last()).toBeNull();
        await fireEvent.click(screen.getByRole('button', { name: 'Crop' }));
        await fireEvent.click(screen.getByRole('button', { name: 'Apply crop' }));
        // change it, then cancel: the applied box stays
        await fireEvent.click(screen.getByRole('button', { name: 'Edit crop' }));
        await fireEvent.keyDown(screen.getByRole('group', { name: /Crop area/ }), { key: 'ArrowLeft', shiftKey: true });
        expect(last()?.crop?.x).toBe(78);
        await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
        expect(last()?.crop).toEqual({ x: 128, y: 72, w: 1024, h: 576 });
        await fireEvent.click(screen.getByRole('button', { name: 'Reset to full capture' }));
        expect(last()).toBeNull();
        expect(screen.queryByRole('button', { name: 'Reset to full capture' })).toBeNull();
      });

      it('moves and resizes the crop box from the keyboard, in even numbers inside the picture', async () => {
        const { last } = await ready();
        await fireEvent.click(screen.getByRole('button', { name: 'Crop' }));
        const box = screen.getByRole('group', { name: /Crop area/ });
        await fireEvent.keyDown(box, { key: 'ArrowLeft', shiftKey: true });
        expect(last()?.crop).toEqual({ x: 78, y: 72, w: 1024, h: 576 });
        await fireEvent.keyDown(box, { key: 'ArrowUp' });
        expect(last()?.crop).toEqual({ x: 78, y: 62, w: 1024, h: 576 });
        await fireEvent.keyDown(box, { key: 'ArrowLeft', altKey: true, shiftKey: true });
        expect(last()?.crop).toEqual({ x: 78, y: 62, w: 974, h: 576 });
        // it cannot leave the picture
        for (let i = 0; i < 30; i++) await fireEvent.keyDown(box, { key: 'ArrowRight', shiftKey: true });
        expect(last()?.crop?.x).toBe(1280 - 974);
        for (let i = 0; i < 30; i++) await fireEvent.keyDown(box, { key: 'ArrowLeft', altKey: true, shiftKey: true });
        expect(last()?.crop?.w).toBe(16);
      });

      it('changes what is shown to the cropped part once it is applied, and back while it is edited', async () => {
        vi.stubGlobal('ResizeObserver', class { constructor(private cb: () => void) {} observe() { this.cb(); } disconnect() {} });
        Object.defineProperty(HTMLElement.prototype, 'clientWidth', { configurable: true, get: () => 400 });
        Object.defineProperty(HTMLElement.prototype, 'clientHeight', { configurable: true, get: () => 400 });
        try {
          await ready();
          const px = (el: HTMLElement, p: 'width' | 'height' | 'left' | 'top') => parseFloat(el.style[p]);
          const picture = () => document.querySelector('.picture') as HTMLElement;
          const frame = () => document.querySelector('.frame') as HTMLElement;
          // the whole picture: 1280 x 720 in a 400 x 400 space
          await waitFor(() => expect(px(picture(), 'width')).toBe(400));
          expect(px(picture(), 'height')).toBeCloseTo(225, 1);
          await fireEvent.click(screen.getByRole('button', { name: 'Crop' }));
          // while the box is set the whole picture is shown
          expect(px(frame(), 'width')).toBeCloseTo(400, 1);
          const box = screen.getByRole('group', { name: /Crop area/ });
          await fireEvent.keyDown(box, { key: 'ArrowUp', altKey: true, shiftKey: true });
          await fireEvent.keyDown(box, { key: 'ArrowUp', altKey: true, shiftKey: true });
          // the box is now 1024 x 476 at (128, 72)
          await fireEvent.click(screen.getByRole('button', { name: 'Apply crop' }));
          const k = 400 / 1024;
          expect(px(picture(), 'width')).toBeCloseTo(400, 1);
          expect(px(picture(), 'height')).toBeCloseTo(476 * k, 1);
          // the whole recording is laid out so that the box fills the picture
          expect(px(frame(), 'width')).toBeCloseTo(1280 * k, 1);
          expect(px(frame(), 'left')).toBeCloseTo(-128 * k, 1);
          expect(px(frame(), 'top')).toBeCloseTo(-72 * k, 1);
        } finally {
          vi.unstubAllGlobals();
          delete (HTMLElement.prototype as unknown as Record<string, unknown>).clientWidth;
          delete (HTMLElement.prototype as unknown as Record<string, unknown>).clientHeight;
        }
      });
    });

    describe('mark', () => {
      it('puts a mark in the middle of the picture, with a text, and takes it away again', async () => {
        const { last } = await ready();
        await fireEvent.click(screen.getByRole('button', { name: 'Mark' }));
        expect(last()).toEqual({ start_ms: 0, end_ms: null, crop: null, mark: true });
        expect(screen.getByRole('group', { name: 'Mark at 640,360. Click the picture or use the arrow keys to move it.' })).toBeTruthy();
        expect(screen.getByText('Keeping all 9.5 s, marked')).toBeTruthy();
        await fireEvent.input(screen.getByLabelText('Text under the mark'), { target: { value: 'Look here' } });
        expect(screen.getByText('Look here')).toBeTruthy();
        await fireEvent.click(screen.getByRole('button', { name: 'Remove mark' }));
        expect(last()).toBeNull();
        expect(screen.queryByLabelText('Text under the mark')).toBeNull();
      });

      it('is placed in the middle of the cropped part when there is one', async () => {
        await ready();
        await fireEvent.click(screen.getByRole('button', { name: 'Crop' }));
        await fireEvent.keyDown(screen.getByRole('group', { name: /Crop area/ }), { key: 'ArrowLeft', shiftKey: true });
        await fireEvent.click(screen.getByRole('button', { name: 'Apply crop' }));
        await fireEvent.click(screen.getByRole('button', { name: 'Mark' }));
        // the crop is 78..1102 across and 72..648 down: its middle is 590,360
        expect(screen.getByRole('group', { name: /^Mark at 590,360/ })).toBeTruthy();
      });

      it('Done keeps the mark, and Edit mark gets it back', async () => {
        const { last } = await ready();
        await fireEvent.click(screen.getByRole('button', { name: 'Mark' }));
        await fireEvent.input(screen.getByLabelText('Text under the mark'), { target: { value: 'x' } });
        await fireEvent.click(screen.getByRole('button', { name: 'Done' }));
        expect(last()?.mark).toBe(true);
        expect(screen.queryByLabelText('Text under the mark')).toBeNull();
        await fireEvent.click(screen.getByRole('button', { name: 'Edit mark' }));
        expect((screen.getByLabelText('Text under the mark') as HTMLInputElement).value).toBe('x');
      });
    });

    describe('crop and mark take turns', () => {
      it('the mark cannot be moved while the crop box has the mouse, and the box is gone while the mark has it', async () => {
        await ready();
        await fireEvent.click(screen.getByRole('button', { name: 'Mark' }));
        const ring = () => screen.getByRole('group', { name: /^Mark at/ });
        expect(ring().getAttribute('tabindex')).toBe('0');
        // starting a crop hands the mouse to the box: the ring stays in view but is not touchable
        await fireEvent.click(screen.getByRole('button', { name: 'Crop' }));
        expect(screen.getByRole('group', { name: /Crop area/ })).toBeTruthy();
        expect(ring().getAttribute('tabindex')).toBe('-1');
        expect(document.querySelector('.layer.idle')).not.toBeNull();
        expect(screen.queryByLabelText('Text under the mark')).toBeNull();
        await fireEvent.click(screen.getByRole('button', { name: 'Apply crop' }));
        // and back: the box goes, the ring can be moved again, in the cropped picture
        await fireEvent.click(screen.getByRole('button', { name: 'Edit mark' }));
        expect(screen.queryByRole('group', { name: /Crop area/ })).toBeNull();
        expect(ring().getAttribute('tabindex')).toBe('0');
      });

      it('starting the mark while the box is open ends the crop mode and keeps the box', async () => {
        const { last } = await ready();
        await fireEvent.click(screen.getByRole('button', { name: 'Crop' }));
        await fireEvent.click(screen.getByRole('button', { name: 'Mark' }));
        // starting the mark ended the crop mode with the box kept
        expect(screen.queryByRole('group', { name: /Crop area/ })).toBeNull();
        expect(last()).toEqual({ start_ms: 0, end_ms: null, crop: { x: 128, y: 72, w: 1024, h: 576 }, mark: true });
      });

      it('resets both with one button', async () => {
        const { last } = await ready();
        await fireEvent.click(screen.getByRole('button', { name: 'Crop' }));
        await fireEvent.click(screen.getByRole('button', { name: 'Apply crop' }));
        await fireEvent.click(screen.getByRole('button', { name: 'Mark' }));
        await fireEvent.input(screen.getByLabelText('Text under the mark'), { target: { value: 'x' } });
        await fireEvent.click(screen.getByRole('button', { name: 'Reset edits' }));
        expect(last()).toBeNull();
        expect(screen.getByRole('button', { name: 'Crop' })).toBeTruthy();
        expect(screen.getByRole('button', { name: 'Mark' })).toBeTruthy();
      });
    });

    describe('playback buttons', () => {
      const button = (name: string) => screen.getByRole('button', { name });

      it('lie over the picture, with the signs people know', async () => {
        await ready();
        const bar = screen.getByRole('toolbar', { name: 'Playback' });
        expect(bar.closest('.picture')).not.toBeNull();
        for (const name of ['Play', 'Stop', 'Slower', 'Faster', 'Mute', 'Full screen']) expect(button(name)).toBeTruthy();
        expect(screen.getByLabelText('Volume')).toBeTruthy();
      });

      it('play and pause, from the button, a click on the picture and the space bar', async () => {
        const { video } = await ready();
        video.play = vi.fn().mockResolvedValue(undefined);
        video.pause = vi.fn();
        Object.defineProperty(video, 'paused', { value: true, configurable: true });
        await fireEvent.click(button('Play'));
        expect(video.play).toHaveBeenCalledTimes(1);
        await fireEvent.click(video);
        expect(video.play).toHaveBeenCalledTimes(2);
        await fireEvent.keyDown(screen.getByRole('group', { name: 'Recording player' }), { key: ' ' });
        expect(video.play).toHaveBeenCalledTimes(3);
        await fireEvent.play(video);
        expect(button('Pause')).toBeTruthy();
        Object.defineProperty(video, 'paused', { value: false, configurable: true });
        await fireEvent.click(button('Pause'));
        expect(video.pause).toHaveBeenCalled();
      });

      it('stop pauses and goes back to the start of the part that is kept', async () => {
        const { video } = await ready();
        video.pause = vi.fn();
        await fireEvent.keyDown(startHandle(), { key: 'ArrowRight', shiftKey: true });
        await playingAt(video, 5, false);
        await fireEvent.click(button('Stop'));
        expect(video.pause).toHaveBeenCalled();
        expect(video.currentTime).toBe(1);
      });

      it('slower and faster step through the speeds and stop at the ends', async () => {
        const { video } = await ready();
        expect(screen.getByText('1×')).toBeTruthy();
        await fireEvent.click(button('Faster'));
        expect(video.playbackRate).toBe(1.5);
        await fireEvent.click(button('Faster'));
        expect(video.playbackRate).toBe(2);
        expect((button('Faster') as HTMLButtonElement).disabled).toBe(true);
        for (let i = 0; i < 4; i++) await fireEvent.click(button('Slower'));
        expect(video.playbackRate).toBe(0.25);
        expect((button('Slower') as HTMLButtonElement).disabled).toBe(true);
        expect(screen.getByText('0.25×')).toBeTruthy();
      });

      it('mute and the volume go to the player, and unmuting never leaves it silent', async () => {
        const { video } = await ready();
        await fireEvent.click(button('Mute'));
        expect(video.muted).toBe(true);
        expect(button('Unmute')).toBeTruthy();
        await fireEvent.click(button('Unmute'));
        expect(video.muted).toBe(false);
        await fireEvent.input(screen.getByLabelText('Volume'), { target: { value: '0' } });
        expect(video.muted).toBe(true);
        await fireEvent.click(button('Unmute'));
        expect(video.muted).toBe(false);
        expect(video.volume).toBe(0.5);
      });

      it('leave out the volume for a recording without sound, and say so', async () => {
        setApiForTests(memoryApi());
        render(Harness, { info: { ...info, audio: false }, onedit: () => {} });
        await waitFor(() => expect(document.querySelector('video')).not.toBeNull());
        expect(screen.queryByRole('button', { name: 'Mute' })).toBeNull();
        expect(screen.queryByLabelText('Volume')).toBeNull();
        expect(screen.getByText('No sound')).toBeTruthy();
        // the other buttons stay
        expect(button('Play')).toBeTruthy();
        expect(button('Full screen')).toBeTruthy();
      });

      it('go full screen with the picture and its buttons, and come back', async () => {
        await ready();
        const stage = screen.getByRole('group', { name: 'Recording player' }) as HTMLElement;
        const request = vi.fn().mockResolvedValue(undefined);
        stage.requestFullscreen = request;
        await fireEvent.click(button('Full screen'));
        expect(request).toHaveBeenCalledTimes(1);
        Object.defineProperty(document, 'fullscreenElement', { value: stage, configurable: true });
        const exit = vi.fn().mockResolvedValue(undefined);
        document.exitFullscreen = exit;
        document.dispatchEvent(new Event('fullscreenchange'));
        expect(await screen.findByRole('button', { name: 'Leave full screen' })).toBeTruthy();
        await fireEvent.click(screen.getByRole('button', { name: 'Leave full screen' }));
        expect(exit).toHaveBeenCalled();
        Object.defineProperty(document, 'fullscreenElement', { value: null, configurable: true });
      });

      it('give way to the crop box while it is being set, and come back after', async () => {
        await ready();
        await fireEvent.click(screen.getByRole('button', { name: 'Crop' }));
        expect(screen.queryByRole('toolbar', { name: 'Playback' })).toBeNull();
        await fireEvent.click(screen.getByRole('button', { name: 'Apply crop' }));
        expect(screen.getByRole('toolbar', { name: 'Playback' })).toBeTruthy();
      });

      it('show where the picture is in the recording', async () => {
        const { video } = await ready();
        await playingAt(video, 3.25);
        expect(screen.getByLabelText('Position').textContent).toBe('3.3 / 9.5 s');
      });
    });
  });
});

describe('VideoBlock mark picture', () => {
  it('draws the mark the size of the recording, where it is and with its text, and nothing without a mark', async () => {
    const draw = vi.spyOn(await import('./drawMark'), 'drawMark').mockResolvedValue(new Blob(['png'], { type: 'image/png' }));
    setApiForTests(memoryApi());
    const { component } = render(VideoBlock, { info });
    await waitFor(() => expect(document.querySelector('video')).not.toBeNull());
    expect(await (component as unknown as { exportMark: () => Promise<Blob | null> }).exportMark()).toBeNull();
    await fireEvent.click(screen.getByRole('button', { name: 'Mark' }));
    await fireEvent.input(screen.getByLabelText('Text under the mark'), { target: { value: 'Here' } });
    const blob = await (component as unknown as { exportMark: () => Promise<Blob | null> }).exportMark();
    expect(blob?.type).toBe('image/png');
    expect(draw).toHaveBeenCalledWith(1280, 720, { x: 640, y: 360 }, 'Here');
    draw.mockRestore();
  });
  describe('Take screenshot', () => {
    /** A canvas that "draws" and hands back a small picture, which jsdom cannot do. */
    function fakeCanvas(blob: Blob | null) {
      const drawn: number[][] = [];
      vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({
        drawImage: (_v: unknown, ...a: number[]) => drawn.push(a),
      } as never);
      vi.spyOn(HTMLCanvasElement.prototype, 'toBlob').mockImplementation((done) => done(blob));
      return drawn;
    }
    afterEach(() => vi.restoreAllMocks());

    it('is not there unless the layout has a screenshot block to put the picture in', async () => {
      setApiForTests(memoryApi());
      render(VideoBlock, { info });
      await screen.findByLabelText('Recording');
      expect(screen.queryByRole('button', { name: 'Take screenshot' })).toBeNull();
    });

    it('hands over the frame on screen, whole, at the size of the recording', async () => {
      setApiForTests(memoryApi());
      const picture = new Blob(['png'], { type: 'image/png' });
      const drawn = fakeCanvas(picture);
      const taken: Blob[] = [];
      render(VideoBlock, { info, onshot: (b: Blob) => taken.push(b) });
      await screen.findByLabelText('Recording');
      await fireEvent.click(screen.getByRole('button', { name: 'Take screenshot' }));
      await waitFor(() => expect(taken).toEqual([picture]));
      expect(drawn).toEqual([[0, 0, 1280, 720]]);
    });

    it('says so when no picture could be made', async () => {
      setApiForTests(memoryApi());
      fakeCanvas(null);
      const taken: Blob[] = [];
      render(VideoBlock, { info, onshot: (b: Blob) => taken.push(b) });
      await screen.findByLabelText('Recording');
      await fireEvent.click(screen.getByRole('button', { name: 'Take screenshot' }));
      expect((await screen.findByRole('alert')).textContent).toContain('could not take a picture');
      expect(taken).toEqual([]);
    });
  });
});

