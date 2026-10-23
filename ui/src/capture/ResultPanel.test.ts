import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, fireEvent } from '@testing-library/svelte';
import ResultPanel from './ResultPanel.svelte';
import type { ShareResult } from '../lib/types';

function show(result: ShareResult) {
  const handlers = {
    onopenlink: vi.fn(),
    oncopy: vi.fn(),
    onreconnect: vi.fn(),
    onclose: vi.fn(),
  };
  render(ResultPanel, { result, ...handlers });
  return handlers;
}

describe('ResultPanel', () => {
  afterEach(cleanup);

  it('offers the link and the second paste after a hand-off', async () => {
    const h = show({
      status: 'sent', summary: 'Created PROJ-1', url: 'https://x.test/PROJ-1',
      notes: ['1 file skipped'], copy_next: 'why?', close: false,
    });
    expect(screen.getByText('Created PROJ-1')).toBeTruthy();
    expect(screen.getByText('1 file skipped')).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: 'Open' }));
    expect(h.onopenlink).toHaveBeenCalledWith('https://x.test/PROJ-1');
    await fireEvent.click(screen.getByRole('button', { name: 'Copy prompt' }));
    expect(h.oncopy).toHaveBeenCalledWith('why?');
    expect(screen.getByRole('button', { name: 'Copied' })).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(h.onclose).toHaveBeenCalled();
  });

  it('says where a local save went', async () => {
    const h = show({
      status: 'saved_locally', path: '/home/me/zips/CargoFrame_2026_local.zip', notes: [], close: false,
    });
    expect(screen.getByText('Saved locally')).toBeTruthy();
    expect(screen.getByText('/home/me/zips/CargoFrame_2026_local.zip')).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(h.onclose).toHaveBeenCalled();
  });

  it('lists what blocks a share', () => {
    show({ status: 'blocked', reasons: ['Write the prompt before sharing', 'Summary needs a value'] });
    expect(screen.getByText('Write the prompt before sharing')).toBeTruthy();
    expect(screen.getByText('Summary needs a value')).toBeTruthy();
  });

  it('says what failed, where the copy went, and offers to reconnect', async () => {
    const h = show({
      status: 'failed', label: 'Sign-in expired', message: 'Jira refused the token', reconnect: true,
      attempts: 1,
      saved: { place: 'safe_folder', path: '/home/me/zips/CargoFrame_x.zip', note: null },
    });
    expect(screen.getByText(/Sign-in expired: Jira refused the token/)).toBeTruthy();
    expect(screen.getByText(/saved in your safe folder: \/home\/me\/zips/)).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: 'Reconnect…' }));
    expect(h.onreconnect).toHaveBeenCalled();
  });

  it('is loud when even the copy could not be saved', () => {
    show({
      status: 'failed', label: 'No network', message: 'offline', reconnect: false, attempts: 4,
      saved: { place: 'not_saved', path: '/data/tmp/keep.zip', note: 'disk full' },
    });
    expect(screen.getByText(/could not be saved. It is still here: \/data\/tmp\/keep.zip/)).toBeTruthy();
    expect(screen.getByText('disk full')).toBeTruthy();
  });
});
