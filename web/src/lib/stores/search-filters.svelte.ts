import type { FilterState } from "$lib/filters";
import { defaultFilters, parseFilters } from "$lib/filters";
import { createContext } from "svelte";

/**
 * Create a reactive FilterState.
 * $state must be used as a variable declaration initializer,
 * so this function declares and returns a local $state variable.
 */
export function createFilterState(
  params?: URLSearchParams,
  validSubjects?: Set<string>,
  resolvedInstructors?: Record<string, string>
): FilterState {
  const state: FilterState = $state(
    params ? parseFilters(params, validSubjects, resolvedInstructors) : defaultFilters()
  );
  return state;
}

export const [getFiltersContext, setFiltersContext] = createContext<FilterState>();
