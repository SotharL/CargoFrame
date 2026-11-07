import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import Harness from './Guide.harness.svelte';
import { guide, startTour, stopTour } from './tourState.svelte';
import { route, go } from '../route.svelte';
import { TOURS } from './tours';

/** A guide of our own, so these tests do not depend on the real ones. */
const demo = {
  id: 'connect-jira',
  title: 'Demo',
  steps: [
    { target: 'one', say: 'First thing.', page: 'rules' as const, click: true },
    { target: 'two', say: 'Second thing.' },
    { target: 'three', say: 'Last thing.' },
  ],
};
const original = TOURS[0]!;

beforeEach(() => {
  go('general');
  TOURS[0] = demo;
});
afterEach(() => {
  cleanup();
  stopTour();
  TOURS[0] = original;
});

describe('Guide', () => {
  it('opens the page of a step, boxes what it points at and says the one thing', async () => {
    render(Harness);
    startTour('connect-jira');
    expect(await screen.findByText('First thing.')).toBeTruthy();
    expect(route.page).toBe('rules');
    await waitFor(() => expect(document.querySelector('.box')).not.toBeNull());
    expect(screen.getByText('1 / 3')).toBeTruthy();
  });

  it('presses the target for a step that says so, and goes on', async () => {
    const pressed: string[] = [];
    render(Harness, { onpress: (n: string) => pressed.push(n) });
    startTour('connect-jira');
    await screen.findByText('First thing.');
    await waitFor(() => expect(document.querySelector('.box')).not.toBeNull());
    await fireEvent.click(screen.getByRole('button', { name: 'Next' }));
    expect(await screen.findByText('Second thing.')).toBeTruthy();
    expect(guide.index).toBe(1);
    // only the step that asks for it presses its target
    expect(pressed).toEqual(['one']);
  });

  it('goes back, ends on the last step, and closes with Esc or the cross', async () => {
    render(Harness);
    startTour('connect-jira');
    await screen.findByText('First thing.');
    await fireEvent.click(screen.getByRole('button', { name: 'Next' }));
    await screen.findByText('Second thing.');
    await fireEvent.click(screen.getByRole('button', { name: 'Back' }));
    expect(guide.index).toBe(0);
    await fireEvent.click(screen.getByRole('button', { name: 'Next' }));
    await screen.findByText('Second thing.');
    await fireEvent.click(screen.getByRole('button', { name: 'Next' }));
    expect(await screen.findByText('Last thing.')).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: 'Done' }));
    expect(guide.tour).toBeNull();

    startTour('connect-jira');
    await screen.findByText('First thing.');
    await fireEvent.keyDown(window, { key: 'Escape' });
    expect(guide.tour).toBeNull();
    startTour('connect-jira');
    await fireEvent.click(await screen.findByRole('button', { name: 'Close the guide' }));
    expect(guide.tour).toBeNull();
  });

  it('says so when the thing is not on the screen yet, and still lets the person go on', async () => {
    render(Harness, { hide: 'two' });
    startTour('connect-jira');
    await fireEvent.click(await screen.findByRole('button', { name: 'Next' }));
    expect(await screen.findByText(/Not on the screen yet/, {}, { timeout: 4000 })).toBeTruthy();
    expect(document.querySelector('.box')).toBeNull();
    await fireEvent.click(screen.getByRole('button', { name: 'Next' }));
    expect(await screen.findByText('Last thing.')).toBeTruthy();
  });
});
