import { describe, expect, it } from 'vitest';
import { stepRate } from './player';

describe('stepRate', () => {
  it('goes one speed up or down and stops at the ends', () => {
    expect(stepRate(1, 1)).toBe(1.5);
    expect(stepRate(1, -1)).toBe(0.5);
    expect(stepRate(2, 1)).toBe(2);
    expect(stepRate(0.25, -1)).toBe(0.25);
  });

  it('starts from the nearest speed when the rate is not one of the steps', () => {
    expect(stepRate(1.2, 1)).toBe(1.5);
    expect(stepRate(1.2, -1)).toBe(1);
    expect(stepRate(9, -1)).toBe(2);
    expect(stepRate(0.1, 1)).toBe(0.25);
  });
});
