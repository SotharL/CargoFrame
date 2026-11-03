import { describe, expect, it } from 'vitest';
import { mount, unmount } from 'svelte';
import Logo from './Logo.svelte';

function render(props: Record<string, unknown>) {
  const target = document.createElement('div');
  document.body.append(target);
  const c = mount(Logo, { target, props });
  return { target, done: () => { void unmount(c); target.remove(); } };
}

describe('Logo', () => {
  it('draws the small mark below 48 px and the large one from 48 px', () => {
    const small = render({ size: 24 });
    expect(small.target.querySelector('svg')!.getAttribute('viewBox')).toBe('0 0 16 16');
    expect(small.target.querySelectorAll('rect[mask]').length).toBe(1);
    small.done();
    const large = render({ size: 64 });
    expect(large.target.querySelector('svg')!.getAttribute('viewBox')).toBe('0 0 64 64');
    expect(large.target.querySelectorAll('rect[mask]').length).toBe(2);
    large.done();
  });

  it('gives the arrow its own class so that the accent colour reaches it, and the frames another', () => {
    const l = render({ size: 32 });
    expect(l.target.querySelector('path.arrow')).not.toBeNull();
    expect(l.target.querySelector('g.frames')).not.toBeNull();
    l.done();
  });

  it('is the mark with the name beside it in the wordmark, at the same height', () => {
    const w = render({ variant: 'wordmark', size: 26 });
    const svg = w.target.querySelector('svg')!;
    expect(svg.getAttribute('viewBox')).toBe('0 0 386.5 64');
    expect(svg.getAttribute('height')).toBe('26');
    expect(Number(svg.getAttribute('width'))).toBeCloseTo((26 * 386.5) / 64, 3);
    expect(w.target.querySelectorAll('path.text').length).toBe(2);
    w.done();
  });

  it('is named for screen readers and its masks never clash with another logo on the page', () => {
    const a = render({ size: 24, label: 'First' });
    const b = render({ size: 24 });
    expect(a.target.querySelector('svg')!.getAttribute('aria-label')).toBe('First');
    expect(b.target.querySelector('svg')!.getAttribute('aria-label')).toBe('CargoFrame');
    const ids = [...document.querySelectorAll('mask')].map((m) => m.id);
    expect(new Set(ids).size).toBe(ids.length);
    a.done();
    b.done();
  });
});
