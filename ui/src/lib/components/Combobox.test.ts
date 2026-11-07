import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import Combobox from './Combobox.svelte';
import type { ComboOption } from '../types';

const options: ComboOption[] = [
  { value: 'priority', label: 'Priority', note: 'priority' },
  { value: 'labels', label: 'Labels', note: 'labels' },
  { value: 'customfield_10042', label: 'Sprint goal', note: 'customfield_10042' },
  { value: 'components', label: 'Components', note: 'components' },
];

const box = () => screen.getByRole('combobox', { name: 'Field' });
const names = () => screen.queryAllByRole('option').map((o) => o.textContent?.replace(/\s+/g, ' ').trim());

describe('Combobox', () => {
  afterEach(cleanup);

  it('lists everything when opened and narrows as you type, by label or by key', async () => {
    render(Combobox, { options, value: '', label: 'Field' });
    await fireEvent.focus(box());
    expect(names()).toHaveLength(4);

    await fireEvent.input(box(), { target: { value: 'spr' } });
    expect(names()).toEqual(['Sprint goal customfield_10042']);

    await fireEvent.input(box(), { target: { value: 'customfield' } });
    expect(names()).toHaveLength(1);

    await fireEvent.input(box(), { target: { value: 'zzz' } });
    expect(screen.getByText('Nothing matches')).toBeTruthy();
  });

  it('picks with the arrow keys and Enter', async () => {
    const onchange = vi.fn();
    render(Combobox, { options, value: '', label: 'Field', onchange });
    await fireEvent.focus(box());
    await fireEvent.keyDown(box(), { key: 'ArrowDown' });
    await fireEvent.keyDown(box(), { key: 'ArrowDown' });
    await fireEvent.keyDown(box(), { key: 'Enter' });
    expect(onchange).toHaveBeenCalledWith('customfield_10042');
    expect((box() as HTMLInputElement).value).toBe('Sprint goal');
    expect(screen.queryByRole('listbox')).toBeNull();
  });

  it('picks with the pointer', async () => {
    const onchange = vi.fn();
    render(Combobox, { options, value: '', label: 'Field', onchange });
    await fireEvent.focus(box());
    await fireEvent.pointerDown(screen.getByRole('option', { name: /Labels/ }));
    expect(onchange).toHaveBeenCalledWith('labels');
  });

  it('closes on Escape and goes back to the chosen value', async () => {
    render(Combobox, { options, value: 'priority', label: 'Field' });
    await fireEvent.focus(box());
    await fireEvent.input(box(), { target: { value: 'lab' } });
    await fireEvent.keyDown(box(), { key: 'Escape' });
    expect(screen.queryByRole('listbox')).toBeNull();
    expect((box() as HTMLInputElement).value).toBe('Priority');
  });

  it('keeps what you typed when free text is allowed', async () => {
    const onchange = vi.fn();
    render(Combobox, { options, value: '', label: 'Field', free: true, onchange });
    await fireEvent.focus(box());
    await fireEvent.input(box(), { target: { value: 'customfield_99999' } });
    expect(onchange).toHaveBeenLastCalledWith('customfield_99999');
    expect(screen.getByText('Use “customfield_99999”')).toBeTruthy();
    await fireEvent.blur(box());
    expect((box() as HTMLInputElement).value).toBe('customfield_99999');
  });

  it('can be disabled', () => {
    render(Combobox, { options, value: '', label: 'Field', disabled: true });
    expect((box() as HTMLInputElement).disabled).toBe(true);
  });
});

describe('Combobox groups and wider search', () => {
  afterEach(cleanup);

  it('shows a heading where the group changes and keeps arrow keys on the options', async () => {
    const grouped: ComboOption[] = [
      { value: 'a', label: 'Alpha', group: 'Recent' },
      { value: 'b', label: 'Beta', group: 'Recent' },
      { value: 'c', label: 'Gamma' },
    ];
    const onchange = vi.fn();
    render(Combobox, { options: grouped, value: '', label: 'Field', onchange });
    await fireEvent.focus(box());
    expect(screen.getAllByText('Recent')).toHaveLength(1);
    expect(names()).toEqual(['Alpha', 'Beta', 'Gamma']);
    await fireEvent.keyDown(box(), { key: 'ArrowDown' });
    await fireEvent.keyDown(box(), { key: 'ArrowDown' });
    await fireEvent.keyDown(box(), { key: 'Enter' });
    expect(onchange).toHaveBeenCalledWith('c');
  });

  it('searches the extra choices only once something is typed', async () => {
    const here: ComboOption[] = [{ value: 'a', label: 'Alpha' }];
    const elsewhere: ComboOption[] = [{ value: 'z', label: 'Zulu', note: 'Other server' }];
    render(Combobox, { options: here, also: elsewhere, value: '', label: 'Field' });
    await fireEvent.focus(box());
    expect(names()).toEqual(['Alpha']);
    await fireEvent.input(box(), { target: { value: 'zul' } });
    expect(names()).toEqual(['Zulu Other server']);
  });
});
