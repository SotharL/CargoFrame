import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
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

    it('starts cropped to the window in focus of a continuous recording, which can be taken away', async () => {
      setApiForTests(memoryApi());
      const seen: (ClipEdit | null)[] = [];
      render(Harness, { info: { ...info, crop: { x: 100, y: 50, w: 640, h: 360 } }, onedit: (t: ClipEdit | null) => seen.push(t) });
      await waitFor(() => expect(document.querySelector('video')).not.toBeNull());
      expect(seen[seen.length - 1]).toEqual({ start_ms: 0, end_ms: null, crop: { x: 100, y: 50, w: 640, h: 360 }, mark: false });
    });

    describe('keys and a window that moved', () => {
      const keys = [
        { from_ms: 1_000, to_ms: 2_000, text: 'Ctrl+S' },
        { from_ms: 3_000, to_ms: 4_500, text: 'hello ⏎' },
      ];
      async function open(more: object) {
        setApiForTests(memoryApi());
        const seen: (ClipEdit | null)[] = [];
        render(Harness, { info: { ...info, ...more }, onedit: (t: ClipEdit | null) => seen.push(t) });
        const video = await waitFor(() => {
          const v = document.querySelector('video');
          expect(v).not.toBeNull();
          return v as HTMLVideoElement;
        });
        return { video, last: () => seen[seen.length - 1] };
      }
      const keysBox = () => screen.getByRole('group', { name: /keys pressed over the recording/i });

      it('shows each line of keys at its time, near the bottom-left corner, and sends them', async () => {
        const { video, last } = await open({ keys });
        // they go on the sent picture, where they are shown: 3% across, 84% down
        expect(last()).toEqual({ start_ms: 0, end_ms: null, crop: null, mark: false, keys: { x: 38, y: 605 } });
        expect(screen.getByText('Keeping all 9.5 s, with the keys')).toBeTruthy();
        for (const [at, text] of [[1.5, 'Ctrl+S'], [3.2, 'hello ⏎']] as const) {
          Object.defineProperty(video, 'currentTime', { value: at, configurable: true });
          await fireEvent.timeUpdate(video);
          expect(keysBox().textContent).toContain(text);
        }
        // between two lines nothing shows but the place, while the mouse is over it
        Object.defineProperty(video, 'currentTime', { value: 2.5, configurable: true });
        await fireEvent.timeUpdate(video);
        expect(keysBox().classList.contains('empty')).toBe(true);
      });

      it('moves with the arrow keys, comes off with its ×, and back with the Keys button', async () => {
        const { last } = await open({ keys });
        await fireEvent.keyDown(keysBox(), { key: 'ArrowUp', shiftKey: true });
        await fireEvent.keyDown(keysBox(), { key: 'ArrowRight' });
        expect(last()?.keys).toEqual({ x: 51, y: 569 });
        await fireEvent.click(screen.getByRole('button', { name: 'Take the keys pressed off the recording' }));
        expect(last()).toBeNull();
        expect(screen.queryByRole('group', { name: /keys pressed over the recording/i })).toBeNull();
        const button = screen.getByRole('button', { name: 'Keys' });
        expect(button.getAttribute('aria-pressed')).toBe('false');
        await fireEvent.click(button);
        expect(last()?.keys).toBeTruthy();
        // Reset edits puts them back where they started
        await fireEvent.click(screen.getByRole('button', { name: 'Reset edits' }));
        expect(last()?.keys).toEqual({ x: 38, y: 605 });
        expect((screen.getByRole('button', { name: 'Reset edits' }) as HTMLButtonElement).disabled).toBe(true);
      });

      it('shows the time of each moment when asked, and sends it where it is put', async () => {
        // 14:03:27.500 local time on the 12th of November 2026
        const started = new Date(2026, 10, 12, 14, 3, 27, 500).getTime();
        const { video, last } = await open({ started_at_ms: started });
        // hidden until asked for
        expect(last()).toBeNull();
        const time = screen.getByRole('button', { name: 'Time' });
        expect(time.getAttribute('aria-pressed')).toBe('false');
        await fireEvent.click(time);
        // above the keys, near the bottom-left corner of the sent picture
        expect(last()).toEqual({ start_ms: 0, end_ms: null, crop: null, mark: false, clock: { x: 38, y: 518 } });
        expect(document.querySelector('.meta')?.textContent).toMatch(/with the time/);
        const box = () => screen.getByRole('group', { name: /time over the recording/i });
        expect(box().textContent).toContain('2026-11-12 14:03:27');
        // a second later on the clip, half a second into the next one... and the one after
        Object.defineProperty(video, 'currentTime', { value: 0.6, configurable: true });
        await fireEvent.timeUpdate(video);
        expect(box().textContent).toContain('14:03:28');
        Object.defineProperty(video, 'currentTime', { value: 1.6, configurable: true });
        await fireEvent.timeUpdate(video);
        expect(box().textContent).toContain('14:03:29');
        await fireEvent.click(screen.getByRole('button', { name: 'Take the time off the recording' }));
        expect(last()).toBeNull();
      });

      it('starts with the time shown when the layout asks, and has no Time button without a start', async () => {
        setApiForTests(memoryApi());
        const seen: (ClipEdit | null)[] = [];
        render(Harness, { info: { ...info, started_at_ms: 1_000 }, clockShown: true, onedit: (t: ClipEdit | null) => seen.push(t) });
        await waitFor(() => expect(seen[seen.length - 1]?.clock).toEqual({ x: 38, y: 518 }));
        cleanup();
        await open({});
        expect(screen.queryByRole('button', { name: 'Time' })).toBeNull();
      });

      it('has no keys button when no key was pressed', async () => {
        const { last } = await open({ keys: [] });
        expect(last()).toBeNull();
        expect(screen.queryByRole('button', { name: 'Keys' })).toBeNull();
      });

      it('follows the window as it was moved, until the crop is set by hand', async () => {
        const follow = [
          { at_ms: 0, x: 100, y: 50 },
          { at_ms: 2_000, x: 100, y: 50 },
          { at_ms: 3_000, x: 300, y: 150 },
        ];
        const { video, last } = await open({ crop: { x: 100, y: 50, w: 640, h: 360 }, follow });
        expect(last()).toEqual({ start_ms: 0, end_ms: null, crop: { x: 100, y: 50, w: 640, h: 360 }, mark: false, follow: true });
        expect(document.querySelector('.meta')?.textContent).toMatch(/cropped to 640×360 following the window/);
        const follows = screen.getByRole('button', { name: 'Follow window' });
        expect(follows.getAttribute('aria-pressed')).toBe('true');
        // half-way through the move, the crop is half-way; turned off, it stays there
        Object.defineProperty(video, 'currentTime', { value: 2.5, configurable: true });
        await fireEvent.timeUpdate(video);
        await fireEvent.click(follows);
        expect(last()).toEqual({ start_ms: 0, end_ms: null, crop: { x: 200, y: 100, w: 640, h: 360 }, mark: false });
        await fireEvent.click(follows);
        expect(last()?.follow).toBe(true);
        // a crop set by hand does not follow
        await fireEvent.click(screen.getByRole('button', { name: 'Edit crop' }));
        expect(follows.isConnected).toBe(false);
        await fireEvent.click(screen.getByRole('button', { name: 'Apply crop' }));
        expect(last()?.follow).toBeUndefined();
      });
    });

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
      it('waits for a box to be drawn, then what it shows is what is sent, and Apply keeps it', async () => {
        const { last } = await ready();
        await fireEvent.click(screen.getByRole('button', { name: 'Crop' }));
        // nothing chosen yet: nothing to apply
        expect((screen.getByRole('button', { name: 'Apply crop' }) as HTMLButtonElement).disabled).toBe(true);
        expect(screen.getByText('Drag over the picture to choose the part to keep')).toBeTruthy();
        await fireEvent.click(screen.getByRole('button', { name: 'Start from the middle' }));
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
        await fireEvent.click(screen.getByRole('button', { name: 'Start from the middle' }));
        await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
        expect(last()).toBeNull();
        await fireEvent.click(screen.getByRole('button', { name: 'Crop' }));
        await fireEvent.click(screen.getByRole('button', { name: 'Start from the middle' }));
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
        await fireEvent.click(screen.getByRole('button', { name: 'Start from the middle' }));
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
          // drawn with the mouse: from (100, 100) to (300, 200) of the 400 x 225 picture, 87.5 px down
          const layer = document.querySelector('.layer') as HTMLElement;
          layer.getBoundingClientRect = () => ({ left: 0, top: 0, width: 400, height: 400, right: 400, bottom: 400, x: 0, y: 0, toJSON() {} });
          await fireEvent.mouseDown(layer, { button: 0, clientX: 100, clientY: 87.5 + 50 });
          await fireEvent.mouseMove(window, { clientX: 300, clientY: 87.5 + 150 });
          await fireEvent.mouseUp(window);
          expect(screen.getByText(/cropped to 640×320/)).toBeTruthy();
          await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
          await fireEvent.click(screen.getByRole('button', { name: 'Crop' }));
          await fireEvent.click(screen.getByRole('button', { name: 'Start from the middle' }));
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

    describe('marks', () => {
      /** The marks' canvas laid over the whole 1280 x 720 recording, one CSS pixel per pixel. */
      function canvasAt() {
        const c = screen.getByLabelText(/^Picture to mark/) as HTMLCanvasElement;
        c.getBoundingClientRect = () => ({ left: 0, top: 0, width: 1280, height: 720, right: 1280, bottom: 720, x: 0, y: 0, toJSON() {} });
        return c;
      }
      async function drag(c: HTMLElement, from: [number, number], to: [number, number]) {
        await fireEvent.pointerDown(c, { button: 0, clientX: from[0], clientY: from[1], pointerId: 1 });
        await fireEvent.pointerMove(c, { clientX: to[0], clientY: to[1], pointerId: 1 });
        await fireEvent.pointerUp(c, { clientX: to[0], clientY: to[1], pointerId: 1 });
      }

      it('draws circles, arrows and the rest with the chosen tool, and takes them away again', async () => {
        const { last } = await ready();
        await fireEvent.click(screen.getByRole('button', { name: 'Mark' }));
        const bar = screen.getByRole('toolbar', { name: 'Marks on the recording' });
        for (const name of ['Draw', 'Square', 'Circle', 'Arrow', 'Text', 'Colour and thickness', 'Undo', 'Redo']) {
          expect(within(bar).getByRole('button', { name })).toBeTruthy();
        }
        // nothing drawn yet: nothing changes what is sent
        expect(last()).toBeNull();
        await drag(canvasAt(), [500, 300], [700, 420]);
        expect(last()).toEqual({ start_ms: 0, end_ms: null, crop: null, mark: true });
        expect(screen.getByText('Keeping all 9.5 s, marked')).toBeTruthy();
        await fireEvent.click(within(bar).getByRole('button', { name: 'Arrow' }));
        await drag(canvasAt(), [100, 100], [400, 300]);
        expect(screen.getByLabelText('Picture to mark, 2 marks')).toBeTruthy();
        await fireEvent.click(within(bar).getByRole('button', { name: 'Undo' }));
        expect(screen.getByLabelText('Picture to mark, 1 mark')).toBeTruthy();
        await fireEvent.click(screen.getByRole('button', { name: 'Remove marks' }));
        expect(last()).toBeNull();
      });

      it('writes a text where it is clicked, and Done keeps the marks for Edit marks', async () => {
        const { last } = await ready();
        await fireEvent.click(screen.getByRole('button', { name: 'Mark' }));
        await fireEvent.click(screen.getByRole('button', { name: 'Text' }));
        await fireEvent.pointerDown(canvasAt(), { button: 0, clientX: 200, clientY: 200, pointerId: 1 });
        const box = await screen.findByRole('textbox', { name: 'Text to add on the recording' });
        await fireEvent.input(box, { target: { value: 'Look here' } });
        await fireEvent.keyDown(box, { key: 'Enter' });
        expect(last()?.mark).toBe(true);
        await fireEvent.click(screen.getByRole('button', { name: 'Done' }));
        expect(screen.queryByRole('toolbar', { name: 'Marks on the recording' })).toBeNull();
        await fireEvent.click(screen.getByRole('button', { name: 'Edit marks' }));
        expect(screen.getByLabelText('Picture to mark, 1 mark')).toBeTruthy();
      });
    });

    describe('crop and marks take turns', () => {
      it('the marks are not drawn on while the crop box has the mouse', async () => {
        await ready();
        await fireEvent.click(screen.getByRole('button', { name: 'Mark' }));
        await fireEvent.click(screen.getByRole('button', { name: 'Crop' }));
        expect(screen.queryByRole('toolbar', { name: 'Marks on the recording' })).toBeNull();
        await fireEvent.click(screen.getByRole('button', { name: 'Start from the middle' }));
        expect(screen.getByRole('group', { name: /Crop area/ })).toBeTruthy();
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
  it('draws the marks the size of the recording, and nothing without a mark', async () => {
    const draw = vi.spyOn(await import('./drawMark'), 'drawMarks').mockResolvedValue(new Blob(['png'], { type: 'image/png' }));
    setApiForTests(memoryApi());
    const { component } = render(VideoBlock, { info });
    await waitFor(() => expect(document.querySelector('video')).not.toBeNull());
    const exportMark = () => (component as unknown as { exportMark: () => Promise<Blob | null> }).exportMark();
    expect(await exportMark()).toBeNull();
    await fireEvent.click(screen.getByRole('button', { name: 'Mark' }));
    const c = screen.getByLabelText(/^Picture to mark/) as HTMLCanvasElement;
    c.getBoundingClientRect = () => ({ left: 0, top: 0, width: 1280, height: 720, right: 1280, bottom: 720, x: 0, y: 0, toJSON() {} });
    await fireEvent.pointerDown(c, { button: 0, clientX: 600, clientY: 300, pointerId: 1 });
    await fireEvent.pointerMove(c, { clientX: 700, clientY: 400, pointerId: 1 });
    await fireEvent.pointerUp(c, { clientX: 700, clientY: 400, pointerId: 1 });
    const blob = await exportMark();
    expect(blob?.type).toBe('image/png');
    const [w, h, marks] = draw.mock.calls[0]!;
    expect([w, h]).toEqual([1280, 720]);
    expect(marks[0]).toMatchObject({ kind: 'ellipse', a: { x: 600, y: 300 }, b: { x: 700, y: 400 } });
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

