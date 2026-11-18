import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/svelte';
import DataPage from './FilesPage.svelte';
import { memoryApi, setApiForTests } from '../../lib/ipc';

beforeEach(() => setApiForTests(memoryApi()));
afterEach(() => {
  cleanup();
  setApiForTests(undefined);
});

describe('Files & backup', () => {
  it('shows where a failed share is saved, the backup, then what is kept here and the reset', async () => {
    render(DataPage);
    await screen.findByRole('heading', { name: 'Kept on this computer' });
    await screen.findByRole('heading', { name: 'When a share fails' });
    const titles = [...document.querySelectorAll('section > h2')].map((h) => h.textContent);
    expect(titles).toEqual(['When a share fails', 'Export and import', 'Kept on this computer']);
  });
});
