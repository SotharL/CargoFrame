// The overflow check: a real browser (the system's Chrome) opens every Settings page and the
// CargoFrame window with the demo data, at the smallest size the app allows and at a large one,
// light and dark, and fails when something sticks out of the box it sits in, or a page scrolls
// sideways. Layout bugs of that kind (a name box past its card, a list cut by the block under it)
// reached the owner before this; behaviour tests cannot see them.
//
//   node scripts/overflow-check.mjs            # after `npm run build`
//   CHROME=/path/to/chrome node scripts/overflow-check.mjs
//
// A problem is printed as: page, size, theme, the element, the box it should stay in, by how much.
//
// It also fails when a page asks for a file the build does not have (a 404 here): in the app,
// Tauri answers such a request with "ERROR tauri::manager: asset not found" in the log, as
// Windows' web view did for `/favicon.ico` on every window it opened before 0.7.3.
import { existsSync } from 'node:fs';
import { preview } from 'vite';
import { chromium } from 'playwright-core';

const SETTINGS = { w: 960, h: 600 }; // app/src/windowing.rs MIN_SETTINGS_WIDTH/HEIGHT
const CAPTURE = { w: 640, h: 420 }; // the CargoFrame window's minimum size
const LARGE = { w: 1440, h: 900 };

/** Each page and state checked: a path, and the steps that bring the state up. */
const VIEWS = [
  { name: 'Connections', path: 'settings.html?page=connections' },
  { name: 'Layouts', path: 'settings.html?page=layouts' },
  { name: 'Layout builder', path: 'settings.html?page=layouts', steps: [['role', 'button', 'Start from a template'], ['role', 'button', 'Use']] },
  { name: 'Fill a field', path: 'settings.html?page=layouts/fields', steps: [['role', 'button', 'Fill a field']] },
  { name: 'Shortcuts', path: 'settings.html?page=triggers' },
  { name: 'Data parsing', path: 'settings.html?page=rules' },
  { name: 'New value', path: 'settings.html?page=rules', steps: [['role', 'button', 'New value: select it in a file']] },
  // a word clicked in the file: the panel with the value's name, how it is found, what each file gives
  { name: 'New value, a selection', path: 'settings.html?page=rules', steps: [['role', 'button', 'New value: select it in a file'], ['text', 'Spawned']] },
  { name: 'Recording', path: 'settings.html?page=video' },
  { name: 'Preferences', path: 'settings.html?page=preferences' },
  { name: 'Files & backup', path: 'settings.html?page=files' },
  { name: 'System', path: 'settings.html?page=system' },
  // debug mode on, the bug report set up, this PC checked: the longest System page
  {
    name: 'System, debug mode',
    path: 'settings.html?page=system',
    steps: [['text', 'Describe CargoFrame in a capture of its own windows'], ['role', 'button', 'Set up the bug report'], ['role', 'button', 'Check this PC']],
  },
  // set by an organisation: the chip in place of Edit and Delete, the locked switches
  { name: 'Connections, managed', path: 'settings.html?page=connections&managed=1' },
  { name: 'Shortcuts, managed', path: 'settings.html?page=triggers&managed=1' },
  { name: 'System, managed', path: 'settings.html?page=system&managed=1' },
  { name: 'CargoFrame window', path: 'capture.html', capture: true },
  // Tags with a list of the service, opened, over a Description block (#55)
  {
    name: 'CargoFrame window, Tags list open',
    path: `capture.html?blocks=${encodeURIComponent(JSON.stringify([
      { id: 'labels', type: 'tags', rect: { x: 0, y: 0, w: 6, h: 2 }, title: 'Labels', tags: ['crash'], suggest: { connection_id: 'jira', options_id: 'labels' } },
      { id: 'note', type: 'text', rect: { x: 0, y: 2, w: 6, h: 3 }, title: 'Description', placement: 'above', role: 'plain', default_text: '' },
      { id: 'shot', type: 'screenshot', rect: { x: 6, y: 0, w: 6, h: 5 } },
    ]))}`,
    capture: true,
    steps: [['focus', '#tags-labels']],
  },
];

/** Run in the page: what sticks out. */
function findOverflow() {
  const out = [];
  const doc = document.documentElement;
  if (doc.scrollWidth > doc.clientWidth + 1) {
    out.push({ what: 'the page', box: 'the window', by: doc.scrollWidth - doc.clientWidth });
  }
  const describe = (el) => {
    const name = el.getAttribute('aria-label') || (el.textContent || '').replace(/\s+/g, ' ').trim().slice(0, 40);
    const cls = typeof el.className === 'string' && el.className ? `.${el.className.trim().split(/\s+/).slice(0, 2).join('.')}` : '';
    return `<${el.tagName.toLowerCase()}${cls}>${name ? ` “${name}”` : ''}`;
  };
  const transparent = (c) => c === 'transparent' || /rgba\(.*,\s*0\)$/.test(c);
  // a box: what the eye reads as the edge something must stay inside
  const isBox = (s) => (parseFloat(s.borderLeftWidth) > 0 && s.borderLeftStyle !== 'none') || !transparent(s.backgroundColor);
  const clips = (s) => s.overflowX !== 'visible';
  const floats = (s) => s.position === 'fixed' || s.position === 'absolute';
  for (const el of document.querySelectorAll('input, button, select, textarea, label, code, h1, h2, h3, h4, p, a, li, .chip')) {
    const r = el.getBoundingClientRect();
    if (r.width === 0 || r.height === 0) continue;
    if (floats(getComputedStyle(el))) continue;
    let box = null;
    for (let p = el.parentElement; p && p !== document.body; p = p.parentElement) {
      const s = getComputedStyle(p);
      if (floats(s)) break; // a list or tooltip over the page: placed on purpose
      if (clips(s)) break; // a box that scrolls or cuts its content: meant
      if (isBox(s)) {
        box = p;
        break;
      }
    }
    if (!box) continue;
    const b = box.getBoundingClientRect();
    const by = Math.max(r.right - b.right, b.left - r.left);
    if (by > 1) out.push({ what: describe(el), box: describe(box), by: Math.round(by) });
  }
  // the CargoFrame window: a block whose content does not fit scrolls, as the Tags block did
  // when its list opened inside it (#55)
  for (const cell of document.querySelectorAll('section.cell')) {
    const over = cell.scrollHeight - cell.clientHeight;
    if (over > 1) out.push({ what: `the content of ${describe(cell.firstElementChild ?? cell)}`, box: 'its block', by: over });
  }
  return out;
}

const chrome =
  process.env.CHROME ||
  ['/usr/bin/google-chrome', '/usr/bin/google-chrome-stable', '/usr/bin/chromium', '/usr/bin/chromium-browser'].find((p) => existsSync(p));
if (!chrome) {
  console.error('overflow check: no Chrome found (set CHROME=/path/to/chrome)');
  process.exit(2);
}

const server = await preview({ preview: { port: 4179, strictPort: true }, logLevel: 'silent' });
const base = 'http://127.0.0.1:4179/';
const browser = await chromium.launch({ executablePath: chrome, args: ['--no-sandbox'] });
let problems = 0;
try {
  for (const view of VIEWS) {
    for (const size of [view.capture ? CAPTURE : SETTINGS, LARGE]) {
      for (const scheme of ['light', 'dark']) {
        const page = await browser.newPage({ viewport: { width: size.w, height: size.h }, colorScheme: scheme });
        const missing = new Set();
        page.on('response', (r) => {
          if (r.url().startsWith(base) && r.status() >= 400) missing.add(new URL(r.url()).pathname);
        });
        await page.addInitScript(() => localStorage.setItem('cargoframe.tour.done', '1'));
        await page.goto(base + view.path);
        await page.waitForLoadState('networkidle');
        for (const [kind, a, b] of view.steps ?? []) {
          if (kind === 'role') await page.getByRole(a, { name: b }).first().click({ timeout: 5000 });
          else if (kind === 'focus') await page.locator(a).first().focus();
          else await page.getByText(a).first().click({ timeout: 5000 });
          await page.waitForTimeout(300);
        }
        await page.waitForTimeout(300);
        for (const p of await page.evaluate(findOverflow)) {
          problems += 1;
          console.log(`✗ ${view.name}, ${size.w}×${size.h}, ${scheme}: ${p.what} sticks out of ${p.box} by ${p.by} px`);
        }
        for (const path of missing) {
          problems += 1;
          console.log(`✗ ${view.name}, ${size.w}×${size.h}, ${scheme}: asks for ${path}, which the build does not have`);
        }
        await page.close();
      }
    }
  }
} finally {
  await browser.close();
  server.httpServer.close();
}
if (problems) {
  console.log(`overflow check: ${problems} problem(s)`);
  process.exit(1);
}
console.log(`overflow check: ${VIEWS.length} views, 2 sizes, light and dark: nothing sticks out, no file missing`);
