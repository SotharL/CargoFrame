// The search points at things in the pages' code by their section titles and `data-tour` names.
// When one is renamed or removed, the finding that points at it must change too: these tests
// fail until it does.
// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { isPageId } from './nav';
import { FINDINGS, fold, pagesNamed, search } from './search';

const pages = import.meta.glob('../**/*.svelte', { query: '?raw', import: 'default', eager: true }) as Record<string, string>;
const code = Object.values(pages).join('\n');
/** The titles the pages give their sections, and the headings they write themselves. */
const titles = new Set([
  ...[...code.matchAll(/<Section\s+title="([^"]+)"/g)].map((m) => m[1]!),
  ...[...code.matchAll(/<h2[^>]*>([^<{]+)<\/h2>/g)].map((m) => m[1]!.trim()),
]);

describe('the settings search', () => {
  it('points only at pages, sections and things that exist', () => {
    for (const f of FINDINGS) {
      expect(isPageId(f.page), f.title).toBe(true);
      if (f.section) expect(titles.has(f.section), `${f.title}: section "${f.section}"`).toBe(true);
      if (f.target) expect(code.includes(`data-tour="${f.target}"`), `${f.title}: ${f.target}`).toBe(true);
    }
    expect(new Set(FINDINGS.map((f) => `${f.page}/${f.title}`)).size).toBe(FINDINGS.length);
  });

  it('finds by every word, any case and accents aside, titles first', () => {
    expect(search('')).toEqual([]);
    expect(search('   ')).toEqual([]);
    expect(search('THEME').map((f) => f.title)).toEqual(['Appearance']);
    expect(search('jira').map((f) => f.title).slice(0, 1)).toEqual(['Connections']);
    // both words must be there
    expect(search('jira theme')).toEqual([]);
    // the start of a word is enough
    expect(search('notif').map((f) => f.title)).toEqual(['Notifications']);
    expect(fold('Créer ÉTÉ')).toBe('creer ete');
    // "tags" is a title word for the Tags block, so it comes before blocks that only mention it
    expect(search('tags')[0]?.title).toBe('Tags block');
  });

  it('also finds pages by their own name and description', () => {
    expect(pagesNamed('ffmpeg')).toContain('video');
    expect(pagesNamed('')).toEqual([]);
  });
});
