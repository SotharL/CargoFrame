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
