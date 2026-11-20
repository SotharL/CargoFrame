import { describe, expect, it } from 'vitest';
import { Combination, describePress, keycaps, keyName } from './shortcut';

const press = (code: string, mods: Partial<Record<'ctrlKey' | 'altKey' | 'shiftKey' | 'metaKey', boolean>> = {}) =>
  describePress({ code, ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, ...mods });

describe('describePress', () => {
  it('names a modifier combination by position, not by the typed character', () => {
    expect(press('F9', { ctrlKey: true, shiftKey: true })).toEqual({ text: 'Ctrl+Shift+F9', complete: true });
    expect(press('KeyS', { altKey: true })).toEqual({ text: 'Alt+S', complete: true });
    expect(press('Digit1', { metaKey: true })).toEqual({ text: 'Super+1', complete: true });
  });

  it('waits while only modifiers are down', () => {
    expect(press('ControlLeft', { ctrlKey: true })).toEqual({ text: 'Ctrl+', complete: false });
    expect(press('ShiftRight', { ctrlKey: true, shiftKey: true })).toEqual({ text: 'Ctrl+Shift+', complete: false });
    expect(press('MetaLeft')).toEqual({ text: '', complete: false });
  });

  it('keeps modifier order the way the backend writes it', () => {
    const p = press('KeyA', { metaKey: true, shiftKey: true, altKey: true, ctrlKey: true });
    expect(p.text).toBe('Ctrl+Alt+Shift+Super+A');
  });

  it('knows the named and punctuation keys, and refuses what it cannot name', () => {
    expect(keyName('Space')).toBe('Space');
    expect(keyName('ArrowUp')).toBe('ArrowUp');
    expect(keyName('Backquote')).toBe('Backquote');
    expect(keyName('F24')).toBe('F24');
    expect(keyName('F25')).toBeNull();
    expect(keyName('NumpadAdd')).toBe('NumpadAdd');
    expect(keyName('IntlBackslash')).toBe('IntlBackslash');
    expect(keyName('MediaPlayPause')).toBe('MediaPlayPause');
    expect(keyName('Unidentified')).toBeNull();
  });
});

describe('keycaps', () => {
  it('splits a shortcut for display', () => {
    expect(keycaps('Ctrl+Shift+F9')).toEqual(['Ctrl', 'Shift', 'F9']);
    expect(keycaps('')).toEqual([]);
  });
});

describe('Combination', () => {
  const ev = (code: string, mods: Partial<Record<'ctrlKey' | 'altKey' | 'shiftKey' | 'metaKey', boolean>> = {}) =>
    ({ code, ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, ...mods });

  it('takes every key held together, done when all are let go', () => {
    const c = new Combination();
    c.press(ev('ControlLeft', { ctrlKey: true }));
    expect(c.text).toBe('Ctrl+');
    c.press(ev('KeyS', { ctrlKey: true }));
    c.press(ev('KeyA', { ctrlKey: true }));
    expect(c.text).toBe('Ctrl+A+S');
    expect(c.release('KeyS')).toBe(false);
    expect(c.release('KeyA')).toBe(false);
    expect(c.release('ControlLeft')).toBe(true);
    expect(c.complete).toBe(true);
  });

  it('takes the keys browsers name, and says which it cannot use', () => {
    const c = new Combination();
    c.press(ev('IntlBackslash', { altKey: true }));
    expect(c.text).toBe('Alt+IntlBackslash');
    c.press(ev('Lang1'));
    expect(c.unknown).toBe('Lang1');
    // at most three keys
    const d = new Combination();
    for (const k of ['KeyA', 'KeyB', 'KeyC', 'KeyD']) d.press(ev(k));
    expect(d.text).toBe('A+B+C');
  });
});
