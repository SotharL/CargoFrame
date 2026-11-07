import { describe, expect, it } from 'vitest';
import { isTextQuery, readBetween, writeBetween } from './between';

describe('between', () => {
  it('reads the end of the line after a text', () => {
    expect(readBetween('after: Version:')).toEqual({ before: 'Version:', after: '' });
  });

  it('reads the text before and the text after', () => {
    expect(readBetween('between: build= | ;')).toEqual({ before: 'build=', after: ';' });
    expect(readBetween('BETWEEN: BEGIN | END')).toEqual({ before: 'BEGIN', after: 'END' });
  });

  it('starts empty, and leaves the other instructions alone', () => {
    expect(readBetween('')).toEqual({ before: '', after: '' });
    expect(readBetween('tail: 20')).toBeNull();
    expect(readBetween('version')).toBeNull();
    expect(readBetween('between: no bar')).toBeNull();
  });

  it('writes the shortest instruction for what is filled in', () => {
    expect(writeBetween({ before: 'Version:', after: '' })).toBe('after: Version:');
    expect(writeBetween({ before: ' build= ', after: ' ; ' })).toBe('between: build= | ;');
    expect(writeBetween({ before: '', after: 'END' })).toBe('');
  });

  it('gives back what it wrote', () => {
    for (const b of [{ before: 'a', after: '' }, { before: 'a b', after: 'c d' }]) expect(readBetween(writeBetween(b))).toEqual(b);
  });

  it('knows an instruction of the text reader from a path left over from another format', () => {
    expect(isTextQuery('tail: 20')).toBe(true);
    expect(isTextQuery(' Level: error')).toBe(true);
    expect(isTextQuery('build.number')).toBe(false);
    expect(isTextQuery('')).toBe(false);
  });
});
