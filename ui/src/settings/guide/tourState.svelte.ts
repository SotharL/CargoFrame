// Which guide is showing, and where it is. The page of the step is opened by Guide.svelte.
import { selectorOf, tourById, type Tour } from './tours';

export const guide = $state<{ tour: Tour | null; index: number }>({ tour: null, index: 0 });

/**
 * Start a guide. From a button on the screen (`fromScreen`), it begins at the first step that is
 * already showing, so "Show me how" on an open form does not start by pointing at the Add button.
 */
export function startTour(id: string, fromScreen = false): void {
  const tour = tourById(id);
  if (!tour) return;
  let index = 0;
  if (fromScreen) {
    // a step past the first that is already showing: the form that step one opens is open
    const showing = tour.steps.findIndex((s, i) => i > 0 && document.querySelector(selectorOf(s.target)));
    if (showing > 0) index = showing;
  }
  guide.tour = tour;
  guide.index = index;
}

export function stopTour(): void {
  guide.tour = null;
  guide.index = 0;
}
