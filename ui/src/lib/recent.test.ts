import { beforeEach, describe, expect, it } from 'vitest';
import { KEEP, recentChoices, remember } from './recent';

const c = (n: number) => ({ value: `v${n}`, label: `#c${n}` });

describe('recent choices', () => {
  beforeEach(() => localStorage.clear());

  it('puts the latest choice first and never lists one twice', () => {
    remember('l', 'b', c(1));
    remember('l', 'b', c(2));
    remember('l', 'b', c(1));
    expect(recentChoices('l', 'b').map((r) => r.value)).toEqual(['v1', 'v2']);
  });

  it('keeps the last five', () => {
    for (let i = 1; i <= 8; i++) remember('l', 'b', c(i));
    const kept = recentChoices('l', 'b');
    expect(kept).toHaveLength(KEEP);
    expect(kept[0]!.value).toBe('v8');
    expect(kept[KEEP - 1]!.value).toBe('v4');
  });

  it('keeps each layout and block apart', () => {
    remember('l', 'a', c(1));
    remember('l', 'b', c(2));
    remember('m', 'a', c(3));
    expect(recentChoices('l', 'a')).toEqual([c(1)]);
    expect(recentChoices('l', 'b')).toEqual([c(2)]);
    expect(recentChoices('m', 'a')).toEqual([c(3)]);
  });

  it('ignores damaged storage, empty choices and a missing layout', () => {
    localStorage.setItem('cargoframe.recent.l.b', '{"not":"a list"}');
    expect(recentChoices('l', 'b')).toEqual([]);
    localStorage.setItem('cargoframe.recent.l.b', 'nonsense');
    expect(recentChoices('l', 'b')).toEqual([]);
    remember('l', 'b', { value: '', label: 'x' });
    remember(null, 'b', c(1));
    expect(recentChoices('l', 'b')).toEqual([]);
    expect(recentChoices(null, 'b')).toEqual([]);
  });
});
