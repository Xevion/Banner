import { goto } from "$app/navigation";
import type { SortingState } from "@tanstack/table-core";
import type { SearchFilters } from "$lib/stores/search-filters.svelte";

export interface UseURLSyncOptions {
  filters: SearchFilters;
  selectedTerm: () => string;
  defaultTermSlug: string;
  offset: () => number;
  sorting: () => SortingState;
}

/**
 * Syncs search state to the browser URL with smart history batching.
 * Rapid changes (<2.5s apart) are batched into a single history entry.
 */
export function useURLSync(options: UseURLSyncOptions): void {
  let lastNavigationTime = 0;
  const BATCH_WINDOW_MS = 2500;

  $effect(() => {
    const term = options.selectedTerm();
    const filterParams = options.filters.toURLParams();
    const currentOffset = options.offset();
    const currentSorting = options.sorting();

    const params = filterParams;
    if (currentOffset > 0) params.set("offset", String(currentOffset));
    if (currentSorting.length > 0) {
      params.set("sort_by", currentSorting[0].id);
      params.set("sort_dir", currentSorting[0].desc ? "desc" : "asc");
    }

    const hasOtherParams = params.size > 0;
    if (term !== options.defaultTermSlug || hasOtherParams) {
      params.set("term", term);
    }

    const now = Date.now();
    const shouldBatch = now - lastNavigationTime < BATCH_WINDOW_MS;
    lastNavigationTime = now;

    void goto(`?${params.toString()}`, {
      replaceState: shouldBatch,
      noScroll: true,
      keepFocus: true,
    });
  });
}
