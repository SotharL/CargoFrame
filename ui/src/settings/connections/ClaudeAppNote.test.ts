import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import ClaudeAppNote from './ClaudeAppNote.svelte';
import { memoryApi, setApiForTests, type Api, type MemoryOptions } from '../../lib/ipc';

afterEach(() => {
  cleanup();
  setApiForTests(undefined);
});

/** A stand-in with a Claude connection that opens the desktop app (the note only matters then). */
async function setUp(opts: MemoryOptions): Promise<Api> {
  const api = memoryApi({ setup: 'windows_missing', ...opts });
  await api.saveItem('connection', 'claude', {
    kind: 'claude', name: 'Claude', enabled: true, open_with: 'desktop', send_automatically: false,
  } as never);
  setApiForTests(api);
  return api;
}

describe('ClaudeAppNote', () => {
  it('says nothing when the app is there', async () => {
    await setUp({ claudeApp: 'installed' });
    render(ClaudeAppNote, { pollMs: 5 });
    expect(await screen.findByText('Claude desktop app found')).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Install it' })).toBeNull();
  });

  it('offers to install a missing app, and the download page', async () => {
    const api = await setUp({ claudeApp: 'missing' });
    render(ClaudeAppNote, { pollMs: 5 });
    await fireEvent.click(await screen.findByRole('button', { name: 'Install it' }));
    expect(await screen.findByText('Claude desktop app found')).toBeTruthy();
    expect((await api.getSetup()).items.find((i) => i.id === 'claude_app')?.status).toBe('ok');
  });

  it('is absent on other systems', async () => {
    await setUp({ setup: 'linux_missing' });
    render(ClaudeAppNote, { pollMs: 5 });
    await Promise.resolve();
    expect(screen.queryByText(/Claude desktop app/)).toBeNull();
  });
});
