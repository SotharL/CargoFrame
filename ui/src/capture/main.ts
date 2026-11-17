import '../lib/base.css';
import { mount } from 'svelte';
import App from './App.svelte';
import { getApi } from '../lib/ipc';
import { applyAccent, applyTheme } from '../lib/theme';
import { setLanguage } from '../lib/i18n/index.svelte';

// Apply the saved theme and accent before the first paint to avoid a flash of the wrong ones.
const api = await getApi();
const general = await api.getGeneral();
applyTheme(general.theme);
applyAccent(general.accent);
// the language first, so the window opens in it
await setLanguage((await api.getLanguage().catch(() => ({ code: 'en' }))).code);

mount(App, { target: document.getElementById('app')! });
