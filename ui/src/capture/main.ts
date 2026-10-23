import '../lib/base.css';
import { mount } from 'svelte';
import App from './App.svelte';
import { getApi } from '../lib/ipc';
import { applyTheme } from '../lib/theme';

// Apply the saved theme before the first paint to avoid a flash of the wrong one.
const api = await getApi();
applyTheme((await api.getGeneral()).theme);

mount(App, { target: document.getElementById('app')! });
