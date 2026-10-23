import type { ThemeChoice } from './types';

/** `system` follows the operating system; the other two force a theme. */
export function applyTheme(choice: ThemeChoice, root: HTMLElement = document.documentElement): void {
  if (choice === 'light' || choice === 'dark') {
    root.setAttribute('data-theme', choice);
  } else {
    root.removeAttribute('data-theme');
  }
}
