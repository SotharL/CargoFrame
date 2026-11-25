import '../lib/base.css';
import { mount } from 'svelte';
import App from './App.svelte';
import { getApi } from '../lib/ipc';
import { applyAccent, applyTheme } from '../lib/theme';
import { setLanguage } from '../lib/i18n/index.svelte';
import { afterPaint } from '../lib/paint';
import { startDebugView } from '../lib/debugView';
import { guide } from './guide/tourState.svelte';

// Apply the saved theme and accent before the first paint to avoid a flash of the wrong ones.
const api = await getApi();
const general = await api.getGeneral();
applyTheme(general.theme);
applyAccent(general.accent);
// the language first, so the window opens in it
await setLanguage((await api.getLanguage().catch(() => ({ code: 'en' }))).code);

// A file dropped outside a drop area would replace the page with the file: only the areas that
// take files (they stop the event first) get them.
for (const kind of ['dragover', 'drop'] as const) window.addEventListener(kind, (e) => e.preventDefault());

mount(App, { target: document.getElementById('app')! });
// debug mode: what this window shows, for a capture of CargoFrame itself
startDebugView('settings', !!general.debug, () => ({
  guide: guide.tour ? `${guide.tour.title} (step ${guide.index + 1} of ${guide.tour.steps.length})` : '',
  layout_id: document.querySelector('[data-layout]')?.getAttribute('data-layout') ?? '',
  layout: (document.querySelector('input[data-layout-name]') as HTMLInputElement | null)?.value ?? '',
  block: document.querySelector('.grid .block.selected')?.getAttribute('data-name') ?? '',
}));
// Tell the app the window is usable, for the timings in Settings, System.
afterPaint(() => void api.reportPainted('settings').catch(() => {}));
