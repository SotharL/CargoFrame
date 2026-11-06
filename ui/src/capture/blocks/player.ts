/** The speeds the playback buttons step through; 1 is normal speed. */
export const RATES = [0.25, 0.5, 1, 1.5, 2];

/** The next speed up (`1`) or down (`-1`) from `rate`; it stays at the ends. A rate between two steps goes to the one beyond it. */
export function stepRate(rate: number, direction: 1 | -1): number {
  const eps = 1e-9;
  if (direction === 1) return RATES.find((r) => r > rate + eps) ?? RATES[RATES.length - 1]!;
  return [...RATES].reverse().find((r) => r < rate - eps) ?? RATES[0]!;
}
