// Debug mode (System page, off by default): each window tells the app what is on screen, so a
// capture of CargoFrame itself can say it ({cargoframe.page}, {cargoframe.focus}…) and attach
// the layout and the rules (app/src/debug.rs). What is on screen is read from the page itself
// when something changes (a click, the focus, the window coming forward), so no page has to keep
// it up to date. Nothing is sent while the mode is off.

import { getApi } from './ipc';
import type { DebugView } from './types';

let on = false;
let started = false;
let kind: DebugView['window'] = 'settings';
let extra: () => Partial<DebugView> = () => ({});
let timer: ReturnType<typeof setTimeout> | undefined;

const text = (el: Element | null | undefined, max = 80) => {
  const s = (el?.textContent ?? '').replace(/\s+/g, ' ').trim();
  return s.length > max ? `${s.slice(0, max - 1)}…` : s;
};

/** What a control is called: its label, as a screen reader says it. */
function nameOf(el: Element | null): string {
  if (!el || el === document.body) return '';
  const aria = el.getAttribute('aria-label');
  if (aria) return aria.trim();
  const by = el.getAttribute('aria-labelledby');
  if (by) return by.split(/\s+/).map((id) => text(document.getElementById(id))).join(' ').trim();
  if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement || el instanceof HTMLSelectElement) {
    const label = [...document.querySelectorAll('label')].find((l) => el.id && l.htmlFor === el.id) ?? el.closest('label');
    // the label's own words, not what the control holds
    const words = label ? label.cloneNode(true) as Element : null;
    words?.querySelectorAll('input, select, textarea').forEach((c) => c.remove());
    const own = text(words) || el.getAttribute('placeholder') || el.name;
    if (own) return own;
  }
  return text(el, 60);
}

/** What is on screen now, read from the page. */
export function snapshot(): DebugView {
  const focused = document.activeElement;
  const section = focused?.closest('section, fieldset, [role="dialog"], details');
  const heading = section?.querySelector('h2, h3, legend, summary');
  const tabs = [...document.querySelectorAll('[role="tab"][aria-selected="true"]')].map((t) => text(t)).filter(Boolean);
  return {
    window: kind,
    page: text(document.querySelector('main h1, h1')),
    tab: tabs.join(' › '),
    section: text(heading),
    focus: nameOf(focused),
    ...extra(),
  };
}

function send() {
  if (!on) return;
  clearTimeout(timer);
  timer = setTimeout(() => {
    void getApi()
      .then((api) => api.reportView(snapshot()))
      .catch(() => {});
  }, 200);
}

/** Start reporting from this window (once), when debug mode is on. `more` adds what only the
 *  window knows (the guide showing, the layout in the builder, the block chosen). */
export function startDebugView(window_: DebugView['window'], debug: boolean, more?: () => Partial<DebugView>) {
  kind = window_;
  if (more) extra = more;
  on = debug;
  if (!started) {
    started = true;
    for (const e of ['focusin', 'click', 'keyup'] as const) document.addEventListener(e, send, true);
    window.addEventListener('focus', send);
    // turned on or off in System while this window is open: it follows at once
    void getApi()
      .then((api) => api.onDebugChanged(setDebug))
      .catch(() => {});
  }
  send();
}

/** Debug mode turned on or off from the System page. */
export function setDebug(debug: boolean) {
  on = debug;
  send();
}
