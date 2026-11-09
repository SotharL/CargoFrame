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
    await waitFor(() => expect(screen.queryByRole('button', { name: 'X' })).toBeNull());
    expect(screen.getByRole('button', { name: 'Bluesky' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Jira' })).toBeTruthy();
    // guides that are not about one service stay, above the services, which are folded apart
    const first = screen.getByRole('button', { name: 'Make your first capture' });
    expect(first.closest('details.services')).toBeNull();
    expect(screen.getByRole('button', { name: 'Jira' }).closest('details.services')?.querySelector('summary')?.textContent).toBe('Connect a service');
  });
});
