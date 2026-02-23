/**
 * Reactive query composable for on-demand data fetching triggered by dependency changes.
 *
 * Handles the common pattern of:
 * - Fetching data on mount (or using initial data)
 * - Re-fetching when dependencies change
 * - Debouncing rapid dependency changes
 * - Ignoring stale responses from superseded fetches
 * - Cleaning up timers on destroy
 */

import type { ApiErrorClass } from "$lib/api";
import type Result from "true-myth/result";

export interface UseQueryOptions<T> {
  /** Async function returning a Result */
  fetcher: () => Promise<Result<T, ApiErrorClass>>;
  /** Reactive dependencies — re-fetches when these change */
  deps?: () => unknown[];
  /** Debounce interval in ms (default: 0 = no debounce) */
  debounce?: number;
  /** Initial data (e.g., from load function) */
  initial?: T | null;
  /** Called on successful fetch */
  onSuccess?: (data: T) => void;
  /** Called on error */
  onError?: (error: ApiErrorClass) => void;
}

export interface UseQueryReturn<T> {
  /** The most recent successful data */
  readonly data: T | null;
  /** The most recent error (null if last fetch succeeded) */
  readonly error: ApiErrorClass | null;
  /** Whether a fetch is in progress */
  readonly isLoading: boolean;
  /** Manually trigger a refetch */
  refetch: () => Promise<void>;
}

/**
 * Core query logic as a class for testability.
 * Use `useQuery()` in components for full reactive integration.
 */
export class QueryController<T> {
  // Reactive state
  data = $state<T | null>(null);
  error = $state<ApiErrorClass | null>(null);
  isLoading = $state(false);

  // Configuration
  readonly #fetcher: () => Promise<Result<T, ApiErrorClass>>;
  readonly #debounceMs: number;
  readonly #onSuccess?: (data: T) => void;
  readonly #onError?: (error: ApiErrorClass) => void;

  // Internal state
  #debounceTimer: ReturnType<typeof setTimeout> | undefined;
  #destroyed = false;
  #fetchCounter = 0;

  constructor(options: UseQueryOptions<T>) {
    this.#fetcher = options.fetcher;
    this.#debounceMs = options.debounce ?? 0;
    this.#onSuccess = options.onSuccess;
    this.#onError = options.onError;

    if (options.initial !== undefined && options.initial !== null) {
      this.data = options.initial;
    }
  }

  /**
   * Fetch data immediately. Stale responses from superseded fetches are ignored.
   */
  async fetch(): Promise<void> {
    if (this.#destroyed) return;

    const fetchId = ++this.#fetchCounter;
    this.isLoading = true;

    const result = await this.#fetcher();

    // Ignore if destroyed or superseded by a newer fetch
    if (this.#destroyed || fetchId !== this.#fetchCounter) return;

    if (result.isOk) {
      this.data = result.value;
      this.error = null;
      this.#onSuccess?.(result.value);
    } else {
      this.error = result.error;
      this.#onError?.(result.error);
    }

    this.isLoading = false;
  }

  /**
   * Fetch with debounce. If debounce is 0, fetches immediately.
   * Clears any pending debounce timer before scheduling.
   */
  debouncedFetch(): void {
    if (this.#destroyed) return;

    clearTimeout(this.#debounceTimer);

    if (this.#debounceMs <= 0) {
      void this.fetch();
      return;
    }

    this.#debounceTimer = setTimeout(() => {
      void this.fetch();
    }, this.#debounceMs);
  }

  /** Clean up timers. Call when component unmounts. */
  destroy(): void {
    this.#destroyed = true;
    clearTimeout(this.#debounceTimer);
    this.#debounceTimer = undefined;
  }
}

/**
 * Svelte 5 hook for reactive data fetching with dependency tracking.
 *
 * @example
 * ```svelte
 * const query = useQuery({
 *   fetcher: () => client.searchCourses({ term, query: searchText }),
 *   deps: () => [term, searchText],
 *   debounce: 300,
 * });
 *
 * // Access reactive state
 * {query.data?.results}
 * {#if query.isLoading}Searching...{/if}
 * {#if query.error}Error: {query.error.message}{/if}
 * ```
 */
export function useQuery<T>(options: UseQueryOptions<T>): QueryController<T> {
  const controller = new QueryController(options);
  const hasInitial = options.initial !== undefined && options.initial !== null;
  let isFirstRun = true;

  // Effect for dependency tracking and fetching
  $effect(() => {
    // Track dependencies by calling the deps function
    options.deps?.();

    // Skip the initial fetch when initial data was provided — defer to first dep change
    if (isFirstRun && hasInitial) {
      isFirstRun = false;
      return;
    }
    isFirstRun = false;

    controller.debouncedFetch();
  });

  // Cleanup on destroy
  $effect(() => {
    return () => controller.destroy();
  });

  return controller;
}
