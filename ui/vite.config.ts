/// <reference types="vitest/config" />
import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// One HTML entry per window, so each window loads only its own code.
export default defineConfig({
  plugins: [svelte()],
  base: './',
  clearScreen: false,
  server: { host: '127.0.0.1', port: 1420, strictPort: true },
  build: {
    target: 'es2022',
    cssCodeSplit: true,
    rollupOptions: {
      input: { settings: 'settings.html' },
    },
  },
  test: {
    environment: 'jsdom',
    include: ['src/**/*.test.ts'],
    // Svelte must resolve to its browser build under test.
    alias: [{ find: /^svelte$/, replacement: 'svelte' }],
  },
  resolve: { conditions: ['browser'] },
});
