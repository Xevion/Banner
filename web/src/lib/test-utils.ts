/**
 * Narrow away the `undefined` that `noUncheckedIndexedAccess` adds to an indexed read.
 *
 * A test that indexes into a result already fails if the element is missing; this makes
 * that failure explicit instead of leaving the checker to guess.
 */
export function expectDefined<T>(value: T | undefined, label = "value"): T {
  if (value === undefined) throw new Error(`Expected ${label} to be defined`);
  return value;
}
