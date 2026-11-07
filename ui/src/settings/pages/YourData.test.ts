import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import YourData from './YourData.svelte';
import General from './General.svelte';
import { memoryApi, setApiForTests, type Api } from '../../lib/ipc';
import { deeperKey, recentChoices, remember } from '../../lib/recent';

let api: Api;
let reloads = 0;

beforeEach(() => {
  localStorage.clear();
  reloads = 0;
  api = memoryApi();
  setApiForTests(api);
});
afterEach(() => {
  cleanup();
  setApiForTests(undefined);
});

const open = () => render(YourData, { reload: () => reloads++ });
const row = (name: string) => screen.getByText(name).closest('li')!;
const clearOf = (name: RegExp | string) => screen.getByRole('button', { name }) as HTMLButtonElement;

describe('Your data', () => {
  it('says where everything is kept, and how much each kind takes', async () => {
    open();
    await screen.findByText('Log files');
    expect(screen.getByText('~/.config/CargoFrame')).toBeTruthy();
    expect(row('Log files').textContent).toContain('3 files, 117 KB');
    expect(row('Captures and recordings in progress').textContent).toContain('4 files, 5.1 MB');
    expect(row('Zips kept here').textContent).toContain('Empty');
  });

  it('clears one kind, says what went, and leaves the others', async () => {
    open();
    await screen.findByText('Log files');
    await fireEvent.click(clearOf('Clear Log files'));
    expect(await screen.findByText('Removed 3 files (117 KB).')).toBeTruthy();
    await waitFor(() => expect(row('Log files').textContent).toContain('Empty'));
    expect(row('Captures and recordings in progress').textContent).toContain('4 files');
  });

  it('has nothing to clear for an empty kind', async () => {
    open();
    await screen.findByText('Zips kept here');
    expect(clearOf('Clear Zips kept here').disabled).toBe(true);
    expect(clearOf('Clear Downloaded updates').disabled).toBe(true);
  });

  it('says why when a window or a recording is using the data', async () => {
    api.cleanData = async () => {
      throw 'Close the capture windows first: they are using some of it.';
    };
    open();
    await screen.findByText('Log files');
    await fireEvent.click(clearOf('Clear Log files'));
    expect((await screen.findByText(/Close the capture windows first/)).textContent).toContain('using some of it');
  });

  it('tells when files in use were left', async () => {
    api.cleanData = async () => ({ files: 2, bytes: 2048, left: 1 });
    open();
    await screen.findByText('Log files');
    await fireEvent.click(clearOf('Clear Log files'));
    expect(await screen.findByText('Removed 2 files (2.0 KB). 1 in use was left.')).toBeTruthy();
  });

  it('forgets the remembered choices of every dropdown, and only those', async () => {
    remember('bug', 'where', { value: 'C1', label: '#bugs' });
    remember('bug', deeperKey('where', 'C1'), { value: 'p1', label: 'Slow start' });
    localStorage.setItem('something-else', 'kept');
    open();
    await screen.findByText('2 dropdowns');
    await fireEvent.click(clearOf('Forget the remembered choices'));
    expect(await screen.findByText(/The last choices of every dropdown are forgotten/)).toBeTruthy();
    expect(recentChoices('bug', 'where')).toEqual([]);
    expect(recentChoices('bug', deeperKey('where', 'C1'))).toEqual([]);
    expect(localStorage.getItem('something-else')).toBe('kept');
    expect(clearOf('Forget the remembered choices').disabled).toBe(true);
  });

  it('asks twice before starting again, and the first question can be turned down', async () => {
    open();
    await screen.findByText('Log files');
    await fireEvent.click(screen.getByRole('button', { name: 'Reset CargoFrame…' }));
    expect(screen.getByRole('alert').textContent).toContain('cannot be undone');
    await fireEvent.click(screen.getByRole('button', { name: 'Keep everything' }));
    expect(screen.queryByRole('button', { name: 'Yes, reset CargoFrame' })).toBeNull();
    expect(reloads).toBe(0);
  });

  it('starts again: clears the data and the remembered choices, then reloads the settings window', async () => {
    remember('bug', 'where', { value: 'C1', label: '#bugs' });
    open();
    await screen.findByText('Log files');
    await fireEvent.click(screen.getByRole('button', { name: 'Reset CargoFrame…' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Yes, reset CargoFrame' }));
    await waitFor(() => expect(reloads).toBe(1));
    expect(recentChoices('bug', 'where')).toEqual([]);
    expect((await api.getDataReport()).areas.every((a) => a.files === 0)).toBe(true);
  });

  it('keeps everything and says why when the reset is refused', async () => {
    remember('bug', 'where', { value: 'C1', label: '#bugs' });
    api.resetEverything = async () => {
      throw 'A recording is running: stop it first.';
    };
    open();
    await screen.findByText('Log files');
    await fireEvent.click(screen.getByRole('button', { name: 'Reset CargoFrame…' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Yes, reset CargoFrame' }));
    expect(await screen.findByText('A recording is running: stop it first.')).toBeTruthy();
    expect(reloads).toBe(0);
    expect(recentChoices('bug', 'where')).toHaveLength(1);
  });
});

describe('General: notifications', () => {
  it('has a switch for each kind of notice, both on to begin with, and saves a change', async () => {
    render(General);
    const routine = (await screen.findByRole('switch', { name: 'Say when something happens' })) as HTMLInputElement;
    const problems = screen.getByRole('switch', { name: 'Say when something goes wrong' }) as HTMLInputElement;
    expect(routine.checked && problems.checked).toBe(true);
    await fireEvent.click(routine);
    await waitFor(async () => expect((await api.getGeneral()).notifications).toEqual({ routine: false, problems: true }));
    expect(problems.checked).toBe(true);
  });

  it('shows a saved choice', async () => {
    setApiForTests(memoryApi({ general: { notifications: { routine: true, problems: false } } }));
    render(General);
    expect(((await screen.findByRole('switch', { name: 'Say when something goes wrong' })) as HTMLInputElement).checked).toBe(false);
  });
});
