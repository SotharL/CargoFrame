import { describe, expect, it } from 'vitest';
import { insertAt, looksLikeAddress, makeLink } from './automation';

describe('insertAt', () => {
  it('puts the snippet at the cursor and moves the caret after it', () => {
    expect(insertAt('Hello world', 5, 5, ' there')).toEqual({ text: 'Hello there world', caret: 11 });
  });
  it('replaces a selection', () => {
    expect(insertAt('Build ???', 6, 9, '{{rule.build}}')).toEqual({ text: 'Build {{rule.build}}', caret: 20 });
  });
  it('copes with a cursor outside the text', () => {
    expect(insertAt('ab', 99, 120, 'c')).toEqual({ text: 'abc', caret: 3 });
    expect(insertAt('ab', -4, -1, 'c')).toEqual({ text: 'cab', caret: 1 });
  });
});

describe('makeLink', () => {
  it('writes each style', () => {
    expect(makeLink('markdown', 'the build', 'https://ci.example.com/{{rule.build}}')).toBe('[the build](https://ci.example.com/{{rule.build}})');
    expect(makeLink('jira', 'the build', 'https://x.test')).toBe('[the build|https://x.test]');
    expect(makeLink('plain', 'the build', 'https://x.test')).toBe('the build (https://x.test)');
  });
  it('trims, and falls back to what is there', () => {
    expect(makeLink('markdown', '  ', ' https://x.test ')).toBe('https://x.test');
    expect(makeLink('markdown', 'word', '  ')).toBe('word');
  });
});

describe('looksLikeAddress', () => {
  it('accepts web addresses and an address that starts with a variable', () => {
    expect(looksLikeAddress('https://example.com/a?b=1')).toBe(true);
    expect(looksLikeAddress('{{rule.tracker}}/issues/{{rule.build}}')).toBe(true);
  });
  it('rejects the rest', () => {
    expect(looksLikeAddress('example.com')).toBe(false);
    expect(looksLikeAddress('https://')).toBe(false);
    expect(looksLikeAddress('')).toBe(false);
    expect(looksLikeAddress('https://a b')).toBe(false);
  });
});
