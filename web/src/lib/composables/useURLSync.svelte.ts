import { goto } from "$app/navigation";
import type { SortingState } from "@tanstack/table-core";
import type { SearchFilters } from "$lib/stores/search-filters.svelte";

export interface UseURLSyncOptions {
  filters: SearchFilters;
  selectedTerm: () => string;
  defaultTermSlug: () => string;
  offset: () => number;
  sorting: () => SortingState;
  debounceMs?: number;
}

export interface URLSyncHandle {
  /** Flush any pending debounced navigation immediately. */
  navigateNow: () => void;
}

/**
 * Syncs search state to the browser URL with smart history batching and debounce.
 * Rapid changes (<2.5s apart) are batched into a single history entry.
 * Filter text input changes are debounced (default 300ms) to avoid per-keystroke navigations.
 * Call `navigateNow()` for discrete interactions (pagination, sorting) that should navigate immediately.
 */
export function useURLSync(options: UseURLSyncOptions): URLSyncHandle {
  let lastNavigationTime = 0;
  const BATCH_WINDOW_MS = 2500;
  const DEBOUNCE_MS = options.debounceMs ?? 300;
  let debounceTimer: ReturnType<typeof setTimeout> | undefined;
  let pendingParams: string | null = null;

  function doNavigate(paramString: string) {
    const now = Date.now();
    const shouldBatch = now - lastNavigationTime < BATCH_WINDOW_MS;
    lastNavigationTime = now;

    void goto(`?${paramString}`, {
      replaceState: shouldBatch,
      noScroll: true,
      keepFocus: true,
    });
  }

  $effect(() => {
    const term = options.selectedTerm();
    const defaultTermSlug = options.defaultTermSlug();
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
    if (term !== defaultTermSlug || hasOtherParams) {
      params.set("term", term);
    }

    params.sort();
    const currentParams = new URLSearchParams(window.location.search); // eslint-disable-line svelte/prefer-svelte-reactivity -- non-reactive read of current browser URL
    currentParams.sort();
    if (params.toString() === currentParams.toString()) return;

    const paramString = params.toString();
    pendingParams = paramString;

    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      pendingParams = null;
      doNavigate(paramString);
    }, DEBOUNCE_MS);

    return () => clearTimeout(debounceTimer);
  });

  return {
    navigateNow() {
      if (pendingParams !== null) {
        clearTimeout(debounceTimer);
        const params = pendingParams;
        pendingParams = null;
        doNavigate(params);
      }
    },
  };
}
