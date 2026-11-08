import { describe, expect, it } from 'vitest';
import { formatSize, initialValues, toggleFile } from './values';
import { demoSession } from './demo';

describe('window values', () => {
  it('starts from the rendered texts, first dropdown options and preselected files', async () => {
    const view = await demoSession();
    const v = initialValues(view);
    expect(v.texts['prompt']).toContain('Foo');
    expect(v.dropdowns['chat']).toBe(view.dropdowns['chat']!.selected);
    expect(v.files['logs']).toEqual(
      view.files['logs']!.entries.filter((e) => e.selected).map((e) => e.path),
    );
  });

  it('gives every dropdown an empty second step from the start, so the window can bind it', async () => {
    const view = await demoSession();
    const v = initialValues(view);
    expect(Object.keys(view.dropdowns).length).toBeGreaterThan(0);
    for (const id of Object.keys(view.dropdowns)) expect(v.deeper[id]).toEqual({ value: '', label: '' });
  });

  it('toggles files without duplicates', () => {
    expect(toggleFile(['a'], 'b', true)).toEqual(['a', 'b']);
    expect(toggleFile(['a', 'b'], 'b', true)).toEqual(['a', 'b']);
    expect(toggleFile(['a', 'b'], 'a', false)).toEqual(['b']);
  });

  it('formats sizes for people', () => {
    expect(formatSize(12)).toBe('12 B');
    expect(formatSize(2048)).toBe('2.0 KB');
    expect(formatSize(52 * 1024)).toBe('52 KB');
    expect(formatSize(3 * 1024 * 1024)).toBe('3.0 MB');
  });
});
