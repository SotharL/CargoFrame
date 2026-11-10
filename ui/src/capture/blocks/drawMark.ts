import type { Annotation } from '../editor/model';
import { drawAnnotations } from '../editor/render';

/**
 * The marks drawn over a recording (lines, squares, circles, arrows, text) on a see-through
 * picture the size of the recording, as a PNG. ffmpeg lays it over the kept part of the
 * recording, so what the window shows is what the recording gets.
 */
export function drawMarks(width: number, height: number, marks: readonly Annotation[]): Promise<Blob> {
  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext('2d');
  if (!ctx) return Promise.reject('This window cannot draw the marks');
  drawAnnotations(ctx, marks);
  return new Promise((resolve, reject) => {
    canvas.toBlob((b) => (b ? resolve(b) : reject('The marks could not be made into a picture')), 'image/png');
  });
}
