import { describe, expect, it } from 'vitest';
import { roleHint, roleTag } from './roles';

describe('text roles', () => {
  it('tags only the roles that mean something', () => {
    expect(roleTag('plain')).toBe('');
    expect(roleTag('prompt')).toBe('Prompt');
    expect(roleTag('zip_name')).toBe('Zip name');
  });

  it('explains each role', () => {
    expect(roleHint('plain')).toMatch(/notes/);
    expect(roleHint('prompt')).toMatch(/AI chat/);
    expect(roleHint('zip_name')).toMatch(/zip/);
  });
});
