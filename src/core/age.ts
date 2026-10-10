/**
 * Relative ages for the instrument panel ("12 s ago"), coarse on purpose:
 * the reader wants to know whether something just happened, not the exact
 * second once it is minutes old.
 */

export const agePhrase = (thenMs: number, nowMs: number): string => {
  const seconds = Math.max(0, Math.floor((nowMs - thenMs) / 1000));
  if (seconds < 5) return "just now";
  if (seconds < 60) return `${seconds} s ago`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)} min ago`;
  return `${Math.floor(seconds / 3600)} h ago`;
};
