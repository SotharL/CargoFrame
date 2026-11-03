import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import VideoBlock from './VideoBlock.svelte';
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
});
