import { describe, expect, it } from 'vitest';
import { describePress, keycaps, keyName } from './shortcut';

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
    expect(keyName('NumpadAdd')).toBeNull();
    expect(press('NumpadAdd', { ctrlKey: true })).toEqual({ text: 'Ctrl+', complete: false });
  });
});

describe('keycaps', () => {
  it('splits a shortcut for display', () => {
    expect(keycaps('Ctrl+Shift+F9')).toEqual(['Ctrl', 'Shift', 'F9']);
    expect(keycaps('')).toEqual([]);
  });
});
