import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import TransferSection from './TransferSection.svelte';
import { memoryApi, setApiForTests, type Api } from '../../lib/ipc';
import type { ImportPreview, ImportPreviewItem } from '../../lib/types';

afterEach(() => {
  cleanup();
  setApiForTests(undefined);
});

function item(over: Partial<ImportPreviewItem>): ImportPreviewItem {
  return {
    group: 'layouts', id: 'x', name: 'X', exists: false, existing_name: null,
    error: null, notes: [], can_copy: true, ...over,
  };
}

async function seeded(importFile?: ImportPreview): Promise<Api> {
  const api = memoryApi({ importFile });
  await api.saveItem('rule', 'version', { name: 'Version', format: 'json', file: 'package.json', path: 'version' });
  await api.saveItem('infotype', 'branch', { name: 'Branch', kind: 'text' });
  await api.saveItem('layout', 'bug', { name: 'Bug report', grid: { columns: 12, rows: 8 }, blocks: [] });
  setApiForTests(api);
  return api;
}

describe('Export', () => {
  it('lists the three groups with everything ticked, and exports what is ticked', async () => {
    const api = await seeded();
    const spy = vi.spyOn(api, 'exportSettings');
    render(TransferSection);
    expect(await screen.findByText('Layouts')).toBeTruthy();
    expect(screen.getByText('Data from files')).toBeTruthy();
    expect(screen.getByText('Destination fields')).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: 'Export 3 items…' }));
    await waitFor(() => expect(spy).toHaveBeenCalledTimes(1));
    expect(spy.mock.calls[0]![0]).toEqual([
      { group: 'layouts', ids: ['bug'] },
      { group: 'rules', ids: ['version'] },
      { group: 'infotypes', ids: ['branch'] },
    ]);
    expect(await screen.findByText('Exported')).toBeTruthy();
    expect(screen.getByText('/home/me/Documents/cargoframe-settings.json')).toBeTruthy();
  });

  it('leaves out a group that is unticked and disables the button when nothing is ticked', async () => {
    const api = await seeded();
    const spy = vi.spyOn(api, 'exportSettings');
    render(TransferSection);
    await screen.findByText('Layouts');
    await fireEvent.click(screen.getByRole('checkbox', { name: 'All data from files' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Export 2 items…' }));
    await waitFor(() => expect(spy).toHaveBeenCalled());
    expect(spy.mock.calls[0]![0].map((s) => s.group)).toEqual(['layouts', 'infotypes']);

    await fireEvent.click(screen.getByRole('checkbox', { name: 'All layouts' }));
    await fireEvent.click(screen.getByRole('checkbox', { name: 'All destination fields' }));
    const none = screen.getByRole('button', { name: 'Export 0 items…' }) as HTMLButtonElement;
    expect(none.disabled).toBe(true);
  });

  it('says so when there is nothing to export', async () => {
    setApiForTests(memoryApi());
    render(TransferSection);
    expect(await screen.findByText(/Nothing to export yet/)).toBeTruthy();
  });
});

describe('Import', () => {
  const file: ImportPreview = {
    file_name: 'mine.json',
    items: [
      item({ group: 'layouts', id: 'bug', name: 'Bug report', exists: true, existing_name: 'Bug report' }),
      item({ group: 'layouts', id: 'fresh', name: 'Fresh layout' }),
      item({ group: 'rules', id: 'version', name: 'Version', exists: true, existing_name: 'Version', can_copy: false,
        notes: ['Layouts refer to this rule by its id, so it can only replace.'] }),
      item({ group: 'infotypes', id: 'broken', name: 'Broken', error: 'The file is missing a name' }),
    ],
  };

  it('does nothing when the file dialog is cancelled', async () => {
    setApiForTests(memoryApi());
    render(TransferSection);
    await fireEvent.click(await screen.findByRole('button', { name: 'Choose a file…' }));
    await waitFor(() => expect(screen.getByRole('button', { name: 'Choose a file…' })).toBeTruthy());
    expect(screen.queryByText('mine.json')).toBeNull();
  });

  it('shows what is new, what is already here and what cannot come in, with safe defaults', async () => {
    await seeded(file);
    render(TransferSection);
    await fireEvent.click(await screen.findByRole('button', { name: 'Choose a file…' }));
    expect(await screen.findByText('mine.json')).toBeTruthy();
    expect(screen.getByText('New')).toBeTruthy();
    expect(screen.getAllByText(/Already here/)).toHaveLength(2);
    expect(screen.getByText(/Cannot be imported: The file is missing a name/)).toBeTruthy();
    expect(screen.getByText(/can only replace/)).toBeTruthy();

    // Existing layouts default to a copy, new ones to import, existing rules to leave mine.
    expect((screen.getByLabelText('What to do with Bug report') as HTMLSelectElement).value).toBe('copy');
    expect((screen.getByLabelText('What to do with Fresh layout') as HTMLSelectElement).value).toBe('replace');
    expect((screen.getByLabelText('What to do with Version') as HTMLSelectElement).value).toBe('skip');
    // A data rule cannot be imported as a copy; a broken item has no choice at all.
    const rule = screen.getByLabelText('What to do with Version') as HTMLSelectElement;
    expect([...rule.options].map((o) => o.value)).toEqual(['skip', 'replace']);
    expect(screen.queryByLabelText('What to do with Broken')).toBeNull();
  });

  it('sends the chosen action for each item, then shows what happened', async () => {
    const api = await seeded(file);
    const spy = vi.spyOn(api, 'applyImport');
    render(TransferSection);
    await fireEvent.click(await screen.findByRole('button', { name: 'Choose a file…' }));
    await screen.findByText('mine.json');
    await fireEvent.change(screen.getByLabelText('What to do with Version'), { target: { value: 'replace' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Import 3 items' }));
    await waitFor(() => expect(spy).toHaveBeenCalledTimes(1));
    expect(spy.mock.calls[0]![0]).toEqual([
      { group: 'layouts', id: 'bug', action: 'copy' },
      { group: 'layouts', id: 'fresh', action: 'replace' },
      { group: 'rules', id: 'version', action: 'replace' },
    ]);
    expect(await screen.findByText('Imported 3')).toBeTruthy();
    // The preview is gone once it is applied.
    expect(screen.queryByText('mine.json')).toBeNull();
  });

  it('counts only what will change, and will not import when everything is skipped', async () => {
    await seeded(file);
    render(TransferSection);
    await fireEvent.click(await screen.findByRole('button', { name: 'Choose a file…' }));
    await screen.findByText('mine.json');
    await fireEvent.change(screen.getByLabelText('What to do with Bug report'), { target: { value: 'skip' } });
    await fireEvent.change(screen.getByLabelText('What to do with Fresh layout'), { target: { value: 'skip' } });
    const go = screen.getByRole('button', { name: 'Import 0 items' }) as HTMLButtonElement;
    expect(go.disabled).toBe(true);
  });

  it('cancelling drops the preview and tells the backend to forget the file', async () => {
    const api = await seeded(file);
    const spy = vi.spyOn(api, 'discardImport');
    render(TransferSection);
    await fireEvent.click(await screen.findByRole('button', { name: 'Choose a file…' }));
    await screen.findByText('mine.json');
    await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    await waitFor(() => expect(spy).toHaveBeenCalledTimes(1));
    expect(await screen.findByRole('button', { name: 'Choose a file…' })).toBeTruthy();
  });

  it('shows the reason when the file cannot be read', async () => {
    const api = await seeded();
    api.openImport = async () => {
      throw 'That is not a CargoFrame settings file';
    };
    render(TransferSection);
    await fireEvent.click(await screen.findByRole('button', { name: 'Choose a file…' }));
    expect(await screen.findByText(/not a CargoFrame settings file/)).toBeTruthy();
  });
});
