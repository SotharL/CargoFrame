import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import VideoBlock from './VideoBlock.svelte';
import Harness from './VideoBlock.harness.svelte';
import type { ClipTrim } from '../../lib/types';
import { memoryApi, setApiForTests, type Api } from '../../lib/ipc';

const info = { width: 1280, height: 720, duration_ms: 9_500, bytes: 3 * 1024 * 1024 };
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
      const seen: (ClipTrim | null)[] = [];
      render(Harness, { info, ontrim: (t: ClipTrim | null) => seen.push(t) });
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
      expect((screen.getByRole('button', { name: 'Keep it all' }) as HTMLButtonElement).disabled).toBe(true);
      await fireEvent.keyDown(startHandle(), { key: 'ArrowRight', shiftKey: true });
      expect(last()).toEqual({ start_ms: 1000, end_ms: null });
      await fireEvent.keyDown(endHandle(), { key: 'ArrowLeft' });
      expect(last()).toEqual({ start_ms: 1000, end_ms: 9400 });
      expect(screen.getByText('Keeping 1.0 s to 9.4 s (8.4 s of 9.5 s)')).toBeTruthy();
    });

    it('takes the ends from where the recording is playing', async () => {
      const { video, last } = await ready();
      await playingAt(video, 2);
      await fireEvent.click(screen.getByRole('button', { name: 'Start here' }));
      await playingAt(video, 6.5);
      await fireEvent.click(screen.getByRole('button', { name: 'End here' }));
      expect(last()).toEqual({ start_ms: 2000, end_ms: 6500 });
    });

    it('keeps it all again with one button', async () => {
      const { last } = await ready();
      await fireEvent.keyDown(endHandle(), { key: 'ArrowLeft', shiftKey: true });
      expect(last()).not.toBeNull();
      await fireEvent.click(screen.getByRole('button', { name: 'Keep it all' }));
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
      expect(last()).toEqual({ start_ms: 1000, end_ms: null });
      expect((screen.getByRole('button', { name: 'Start here' }) as HTMLButtonElement).disabled).toBe(true);
    });
  });
});
