// Which guide is showing, and where it is. The page of the step is opened by Guide.svelte.
import { tourById, type Tour } from './tours';

export const guide = $state<{ tour: Tour | null; index: number }>({ tour: null, index: 0 });

/** Start a guide from its first step (the Guides menu is the one way in). */
export function startTour(id: string): void {
  const tour = tourById(id);
  if (!tour) return;
  guide.tour = tour;
  guide.index = 0;
}

export function stopTour(): void {
  guide.tour = null;
  guide.index = 0;
}
