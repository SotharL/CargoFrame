import '../lib/base.css';
import { mount } from 'svelte';
import App from './App.svelte';
import { getApi } from '../lib/ipc';
import { applyAccent, applyTheme } from '../lib/theme';
import { setLanguage } from '../lib/i18n/index.svelte';
import { afterPaint } from '../lib/paint';

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
// Tell the app the window is usable, for the timings in Settings, System.
afterPaint(() => void api.reportPainted('settings').catch(() => {}));
