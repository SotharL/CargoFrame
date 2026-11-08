import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/svelte';
import DataPage from './DataPage.svelte';
import { memoryApi, setApiForTests } from '../../lib/ipc';

beforeEach(() => setApiForTests(memoryApi()));
afterEach(() => {
  cleanup();
  setApiForTests(undefined);
});

describe('Your data page', () => {
  it('shows what is kept here, then the backup', async () => {
    render(DataPage);
    await screen.findByRole('heading', { name: 'Export and import' });
    const titles = [...document.querySelectorAll('section > h2')].map((h) => h.textContent);
    expect(titles).toEqual(['Your data', 'Export and import']);
  });
});
