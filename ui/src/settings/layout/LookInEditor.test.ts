import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import LookInEditor from './LookInEditor.svelte';
import { getApi } from '../../lib/ipc';
import type { Block, FilesPreview } from '../../lib/types';

type Files = Extract<Block, { type: 'files' }>;
const block = (folders: Files['look_in']['folders'] = ['project'], subfolder = ''): Files => ({
  id: 'files', type: 'files', rect: { x: 0, y: 0, w: 6, h: 3 }, title: 'Files',
  include: ['*.log'], preselect: [], look_in: { folders, subfolder },
});

const answer = (over: Partial<FilesPreview> = {}): FilesPreview => ({
  program: 'C:\\Dev\\Foo\\bin\\Foo.exe',
  places: [{ label: 'Project folder', folder: 'C:\\Dev\\Foo', exists: true, found: 3 }],
  missing: [],
  total: 3,
  ticked: 1,
  truncated: false,
  problem: null,
  ...over,
});

describe('LookInEditor', () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it('offers the project folder and the folders above the program, with the project folder ticked', () => {
    render(LookInEditor, { block: block() });
    const boxes = screen.getAllByRole('checkbox') as HTMLInputElement[];
    expect(boxes).toHaveLength(5);
    expect(screen.getByLabelText('The project folder')).toHaveProperty('checked', true);
    expect(screen.getByLabelText('Three folders above')).toHaveProperty('checked', false);
  });

  it('writes the choices into the block, in a fixed set without repeats', async () => {
    const b = block();
    render(LookInEditor, { block: b });
    await fireEvent.click(screen.getByLabelText('Three folders above'));
    await fireEvent.click(screen.getByLabelText("The program's own folder"));
    expect([...b.look_in.folders].sort()).toEqual(['above_3', 'program', 'project']);
    await fireEvent.click(screen.getByLabelText('The project folder'));
    expect([...b.look_in.folders].sort()).toEqual(['above_3', 'program']);
  });

  it('never lets the last folder be unticked', () => {
    render(LookInEditor, { block: block(['above_2']) });
    expect(screen.getByLabelText('Two folders above')).toHaveProperty('disabled', true);
    expect(screen.getByLabelText('The project folder')).toHaveProperty('disabled', false);
  });

  it('treats an empty choice as the project folder', () => {
    render(LookInEditor, { block: block([]) });
    expect(screen.getByLabelText('The project folder')).toHaveProperty('checked', true);
  });

  it('keeps the subfolder the person types', async () => {
    const b = block(['program']);
    render(LookInEditor, { block: b });
    await fireEvent.input(screen.getByPlaceholderText('logs'), { target: { value: 'logs' } });
    expect(b.look_in).toEqual({ folders: ['program'], subfolder: 'logs' });
  });

  it('shows a live count, per folder, for the program of the last capture', async () => {
    const api = await getApi();
    const spy = vi.spyOn(api, 'previewFiles').mockResolvedValue(
      answer({
        places: [
          { label: 'Project folder', folder: 'C:\\Dev\\Foo', exists: true, found: 3 },
          { label: 'Program folder', folder: 'C:\\Dev\\Foo\\bin\\logs', exists: false, found: 0 },
        ],
      }),
    );
    render(LookInEditor, { block: block(['project', 'program'], 'logs') });
    await waitFor(() => expect(screen.getByText('3 files found')).toBeTruthy(), { timeout: 2000 });
    expect(screen.getByText(/1 ticked by default/)).toBeTruthy();
    expect(screen.getByText('Project folder: 3 files')).toBeTruthy();
    expect(screen.getByText('Program folder: folder not found')).toBeTruthy();
    expect(spy).toHaveBeenCalledWith(expect.objectContaining({ look_in: { folders: ['project', 'program'], subfolder: 'logs' } }), null);
  });

  it('tries another program when one is typed', async () => {
    const api = await getApi();
    const spy = vi.spyOn(api, 'previewFiles').mockResolvedValue(answer());
    render(LookInEditor, { block: block() });
    await fireEvent.input(screen.getByRole('textbox', { name: /try with/i }), { target: { value: '  /srv/app/bin/app ' } });
    await waitFor(() => expect(spy).toHaveBeenCalledWith(expect.anything(), '/srv/app/bin/app'), { timeout: 2000 });
  });

  it('says what is needed when there is nothing to count against', async () => {
    const api = await getApi();
    vi.spyOn(api, 'previewFiles').mockResolvedValue(
      answer({ places: [], total: 0, ticked: 0, program: null, problem: 'Make a capture first, or type the path of a program file, to see what is found' }),
    );
    render(LookInEditor, { block: block() });
    await waitFor(() => expect(screen.getByText(/Make a capture first/)).toBeTruthy(), { timeout: 2000 });
  });
});
