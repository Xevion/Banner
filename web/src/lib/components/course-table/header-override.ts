import type { SortKey, SortKeyOption } from "$lib/bindings";
import type { HeaderOverride } from "$lib/components/SortableHeader.svelte";
import { headerSortStep, type SortTerm } from "$lib/sort";

/** What a header needs to know about the table around it to describe itself. */
export interface HeaderContext {
  terms: readonly SortTerm[];
  labels: ReadonlyMap<SortKey, SortKeyOption>;
  isColumnVisible: (id: string) => boolean;
  onSort: (next: SortTerm | null) => void;
}

/**
 * How one course table header reads and what its click does.
 *
 * Every sortable header runs the same cycle: each key the column offers, both
 * ways round, then off. The instructor header's five states are that rule on a
 * two-key column rather than a mechanism of its own.
 */
export function courseHeaderOverride(
  headerId: string,
  context: HeaderContext
): HeaderOverride | null {
  const step = headerSortStep(headerId, context.terms, context.labels);
  if (!step) return null;

  // Shown together the columns are one range under one label, so the titles are
  // left to say which half each sort control orders.
  const paired = context.isColumnVisible("time") && context.isColumnVisible("time_end");
  let label = step.label ?? undefined;
  if (paired && headerId === "time") label = "Time";

  return {
    label,
    labelHidden: paired && headerId === "time_end",
    indicator: step.indicator,
    title: step.title,
    onclick: () => context.onSort(step.next),
  };
}
