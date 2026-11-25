// @vitest-environment node
//
// Each window's page names its icon. Without one, Windows' web view asks for /favicon.ico every
// time a window opens, the app has no such file, and Tauri writes "ERROR tauri::manager: asset
// not found: favicon.ico" to the log (seen on the owner's PC before 0.7.3). A real browser cannot
// show it: Chrome fetches the icon itself, outside what a page or the overflow check sees.
import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';

/** The windows' pages: vite.config.ts builds these two, and `app/src/windowing.rs` opens them. */
const pages = ['settings.html', 'capture.html'];
const read = (name: string) => readFileSync(new URL(`../../${name}`, import.meta.url), 'utf8');

describe('the windows’ pages', () => {
  it('are the ones the build makes', () => {
    for (const page of pages) expect(read('vite.config.ts')).toContain(page);
  });

  for (const page of pages) {
    it(`${page} names an icon that needs no file`, () => {
      const icon = read(page).match(/<link\s+rel="icon"\s+href="([^"]*)"/);
      expect(icon, `${page} has no <link rel="icon">`).not.toBeNull();
      // a data: link is allowed by the windows' CSP (img-src data:) and is not looked up
      expect(icon?.[1]?.startsWith('data:')).toBe(true);
    });
  }
});
