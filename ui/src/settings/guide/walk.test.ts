// Every guide, walked through the real Settings window: each step must find what it points at.
// This is what keeps the guides true when a page changes.
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import App from '../App.svelte';
import { guide, startTour, stopTour } from './tourState.svelte';
import { selectorOf, TOURS } from './tours';
import { go } from '../route.svelte';
import { memoryApi, setApiForTests } from '../../lib/ipc';

beforeEach(() => {
  setApiForTests(memoryApi({ setup: 'windows_missing' }));
  go('general');
});
afterEach(() => {
  stopTour();
  cleanup();
  setApiForTests(undefined);
});

/** What a person would type into the empty boxes of an open form, so that Save can work. */
async function fill() {
  const boxes = [...document.querySelectorAll<HTMLInputElement>('form input[type="text"]')];
  for (const box of boxes) {
    if (box.value) continue;
    const value = box.placeholder.startsWith('http') || box.placeholder.includes('@') || /^[0-9a-f-]{36}$/.test(box.placeholder)
      ? box.placeholder
      : box.placeholder || '3f2a1b4c-5d6e-4f70-8a9b-0c1d2e3f4a5b';
    await fireEvent.input(box, { target: { value } });
  }
}

describe('walking the guides', () => {
  for (const tour of TOURS) {
    it(`${tour.id}: every step finds what it points at`, async () => {
      render(App);
      startTour(tour.id);
      for (const [i, step] of tour.steps.entries()) {
        await waitFor(
          () => {
            const here = [...document.querySelectorAll('[data-tour]')].map((e) => e.getAttribute('data-tour')).join(' ');
            expect(document.querySelector(selectorOf(step.target)), `${step.target} (on the screen: ${here})`).not.toBeNull();
            expect(guide.index).toBe(i);
            // the guide has found it too: no "not on the screen yet"
            expect(document.querySelector('.say')?.textContent).not.toContain('Not on the screen yet');
          },
          { timeout: 4000 },
        );
        if (step.click) await fill();
        // the guide's own buttons, not a form's Done
        const guideBox = within(screen.getByRole('dialog', { name: tour.title }));
        await fireEvent.click(guideBox.getByRole('button', { name: i === tour.steps.length - 1 ? 'Done' : 'Next' }));
      }
      await waitFor(() => expect(guide.tour).toBeNull());
    }, 30_000);
  }
});
