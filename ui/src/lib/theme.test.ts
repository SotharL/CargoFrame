import { describe, expect, it } from 'vitest';
import { applyTheme } from './theme';

describe('applyTheme', () => {
  it('forces light or dark and clears the override for system', () => {
    const root = document.createElement('html');
    applyTheme('dark', root);
    expect(root.getAttribute('data-theme')).toBe('dark');
    applyTheme('light', root);
    expect(root.getAttribute('data-theme')).toBe('light');
    applyTheme('system', root);
    expect(root.hasAttribute('data-theme')).toBe(false);
  });
});

import {
  ACCENT_PRESETS, DEFAULT_ACCENT, accentIsVisible, applyAccent, contrast, normalizeAccent, onAccent,
} from './theme';

describe('accent colour', () => {
  it('accepts #rrggbb and #rgb in any case and refuses the rest', () => {
    expect(normalizeAccent('#4F8EE0')).toBe('#4f8ee0');
    expect(normalizeAccent(' #abc ')).toBe('#aabbcc');
    for (const bad of ['', '4f8ee0', '#4f8ee', '#4f8ee0ff', '#gggggg', 'blue']) {
      expect(normalizeAccent(bad), bad).toBeNull();
    }
  });

  it('measures contrast the way the guidelines do', () => {
    expect(contrast('#000000', '#ffffff')).toBeCloseTo(21, 0);
    expect(contrast('#ffffff', '#ffffff')).toBeCloseTo(1, 5);
  });

  it('puts the dark text on the default blue, as the design system says, and white on a dark accent', () => {
    expect(onAccent(DEFAULT_ACCENT)).toBe('#1f1e1d');
    expect(onAccent('#1d3557')).toBe('#ffffff');
    expect(onAccent('#ffd60a')).toBe('#1f1e1d');
  });

  it('warns about an accent that nearly vanishes on one of the themes', () => {
    expect(accentIsVisible(DEFAULT_ACCENT)).toEqual({ light: true, dark: true });
    expect(accentIsVisible('#fff3a0').light).toBe(false);
    expect(accentIsVisible('#1d3557').dark).toBe(false);
  });

  it('suggests only colours that show on both themes, starting with the default', () => {
    expect(ACCENT_PRESETS[0]!.hex).toBe(DEFAULT_ACCENT);
    for (const p of ACCENT_PRESETS) {
      expect(accentIsVisible(p.hex), p.name).toEqual({ light: true, dark: true });
    }
  });

  it('sets the two colour variables for a chosen accent and clears them for the default or a bad value', () => {
    const root = document.createElement('html');
    applyAccent('#e8590c', root);
    expect(root.style.getPropertyValue('--accent')).toBe('#e8590c');
    expect(root.style.getPropertyValue('--on-accent')).toBe(onAccent('#e8590c'));
    applyAccent(DEFAULT_ACCENT.toUpperCase(), root);
    expect(root.style.getPropertyValue('--accent')).toBe('');
    applyAccent('#102030', root);
    applyAccent('nonsense', root);
    expect(root.style.getPropertyValue('--accent')).toBe('');
    expect(root.style.getPropertyValue('--on-accent')).toBe('');
  });
});
