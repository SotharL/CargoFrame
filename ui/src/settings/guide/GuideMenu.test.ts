import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/svelte';
import GuideMenu from './GuideMenu.svelte';
import { memoryApi, setApiForTests } from '../../lib/ipc';

afterEach(() => {
  cleanup();
  setApiForTests(undefined);
});

describe('Guides menu', () => {
  it('has a guide for every service this copy has, and none for one it was built without', async () => {
    setApiForTests(memoryApi({ connectors: ['jira', 'bluesky'] }));
    render(GuideMenu);
    await waitFor(() => expect(screen.queryByRole('button', { name: 'Connect X' })).toBeNull());
    expect(screen.getByRole('button', { name: 'Connect Bluesky' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Connect Jira' })).toBeTruthy();
    // guides that are not about one service stay
    expect(screen.getByRole('button', { name: 'Make your first capture' })).toBeTruthy();
  });
});
