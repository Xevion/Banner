import type { SortingState, Updater } from "@tanstack/table-core";

/**
 * Create a sorting change handler for TanStack Table.
 *
 * The returned function applies an `Updater<SortingState>` to the current
 * sorting value -- the standard one-liner every table component needs.
 *
 * @param getSorting - Accessor for the current sorting state (called on each update).
 * @param setSorting - Setter to apply the new sorting state.
 */
export function createSortingHandler(
  getSorting: () => SortingState,
  setSorting: (next: SortingState) => void
): (updater: Updater<SortingState>) => void {
  return (updater) => {
    const next = typeof updater === "function" ? updater(getSorting()) : updater;
    setSorting(next);
  };
}
