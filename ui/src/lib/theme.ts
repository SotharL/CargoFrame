import type { ThemeChoice } from './types';

/** `system` follows the operating system; the other two force a theme. */
export function applyTheme(choice: ThemeChoice, root: HTMLElement = document.documentElement): void {
  if (choice === 'light' || choice === 'dark') {
    root.setAttribute('data-theme', choice);
  } else {
    root.removeAttribute('data-theme');
  }
}

/** The accent a new install starts with: the blue of the design system. */
export const DEFAULT_ACCENT = '#4f8ee0';

/** A few suggestions for the accent; each stays visible (3:1) on both the light and the dark page. */
export const ACCENT_PRESETS = [
  { name: 'Blue', hex: '#4f8ee0' },
  { name: 'Orange', hex: '#e8590c' },
  { name: 'Teal', hex: '#1f9d8f' },
  { name: 'Green', hex: '#3d9a4b' },
  { name: 'Purple', hex: '#8b6fe0' },
  { name: 'Pink', hex: '#d6458f' },
];

/** The page backgrounds of the two themes, for telling when an accent is hard to see. */
export const LIGHT_BACKGROUND = '#faf9f5';
export const DARK_BACKGROUND = '#262624';

/** The text colour put on an accent fill when it is dark (the design system's text colour). */
const DARK_TEXT = '#1f1e1d';

/** `#rrggbb` in lower case from `#rrggbb` or `#rgb`; `null` when it is not a colour. */
export function normalizeAccent(value: string): string | null {
  const v = value.trim().toLowerCase();
  if (/^#[0-9a-f]{6}$/.test(v)) return v;
  if (/^#[0-9a-f]{3}$/.test(v)) return `#${v[1]}${v[1]}${v[2]}${v[2]}${v[3]}${v[3]}`;
  return null;
}

function channels(hex: string): [number, number, number] {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

function luminance(hex: string): number {
  const [r, g, b] = channels(hex).map((c) => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  }) as [number, number, number];
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** WCAG contrast ratio between two `#rrggbb` colours (1 to 21). */
export function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number];
  return (hi + 0.05) / (lo + 0.05);
}

/** The text colour that reads best on an accent fill: the design system's dark text, or white. */
export function onAccent(accent: string): string {
  return contrast(accent, DARK_TEXT) >= contrast(accent, '#ffffff') ? DARK_TEXT : '#ffffff';
}

/** Whether an accent stays visible (3:1, the bar for controls and focus rings) on both themes. */
export function accentIsVisible(accent: string): { light: boolean; dark: boolean } {
  return {
    light: contrast(accent, LIGHT_BACKGROUND) >= 3,
    dark: contrast(accent, DARK_BACKGROUND) >= 3,
  };
}

/**
 * Colours the buttons, focus rings, links, the selected block and the arrow of the logo. The
 * default accent leaves the stylesheet's own values in place; anything that is not a colour
 * counts as the default.
 */
export function applyAccent(value: string, root: HTMLElement = document.documentElement): void {
  const accent = normalizeAccent(value);
  if (!accent || accent === DEFAULT_ACCENT) {
    root.style.removeProperty('--accent');
    root.style.removeProperty('--on-accent');
    return;
  }
  root.style.setProperty('--accent', accent);
  root.style.setProperty('--on-accent', onAccent(accent));
}
