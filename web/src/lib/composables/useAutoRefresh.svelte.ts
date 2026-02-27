/**
 * Auto-refreshing data fetcher with exponential backoff on errors.
 *
 * Handles the common pattern of:
 * - Fetching data on mount
 * - Re-fetching when dependencies change
 * - Auto-refreshing on an interval
 * - Backing off when errors occur
 * - Cleaning up timers on destroy
 */

import type { ApiErrorClass } from "$lib/api";
import type Result from "true-myth/result";

export interface AutoRefreshOptions<T> {
  /** Async function returning a Result */
  fetcher: () => Promise<Result<T, ApiErrorClass>>;
  /** Base refresh interval in ms (default: 5000). Set to 0 to disable auto-refresh. */
  interval?: number;
  /** Max interval after repeated errors (default: 60000) */
  maxInterval?: number;
  /** Minimum time to show loading state in ms (default: 0) */
  minLoadingMs?: number;
  /** Start paused - call resume() to begin fetching (default: false) */
  paused?: boolean;
}

export interface AutoRefreshHookOptions<T> extends AutoRefreshOptions<T> {
  /** Dependencies that trigger re-fetch when changed (optional) */
  deps?: () => unknown[];
}

const DEFAULT_INTERVAL = 5_000;
const DEFAULT_MAX_INTERVAL = 60_000;

/**
 * Core auto-refresh logic as a class for testability.
 * Use `useAutoRefresh()` in components for full reactive integration.
 */
export class AutoRefreshController<T> {
  // Reactive state
  data = $state<T | null>(null);
  error = $state<ApiErrorClass | null>(null);
  isLoading = $state(false);

  // Configuration
  readonly #baseInterval: number;
  readonly #maxInterval: number;
  readonly #minLoadingMs: number;
  readonly #fetcher: () => Promise<Result<T, ApiErrorClass>>;

  // Internal state
  #currentInterval: number;
  #refreshTimer: ReturnType<typeof setTimeout> | undefined;
  #loadingHoldTimer: ReturnType<typeof setTimeout> | undefined;
  #destroyed = false;
  #isPaused: boolean;
  #fetchInProgress = false;

  constructor(options: AutoRefreshOptions<T>) {
    this.#baseInterval = options.interval ?? DEFAULT_INTERVAL;
    this.#maxInterval = options.maxInterval ?? DEFAULT_MAX_INTERVAL;
    this.#minLoadingMs = options.minLoadingMs ?? 0;
    this.#fetcher = options.fetcher;
    this.#currentInterval = this.#baseInterval;
    this.#isPaused = options.paused ?? false;
  }

  get hasError(): boolean {
    return this.error !== null;
  }

  get isPaused(): boolean {
    return this.#isPaused;
  }

  #clearTimers(): void {
    clearTimeout(this.#refreshTimer);
    clearTimeout(this.#loadingHoldTimer);
    this.#refreshTimer = undefined;
    this.#loadingHoldTimer = undefined;
  }

  #scheduleRefresh(): void {
    if (this.#destroyed || this.#isPaused || this.#baseInterval <= 0) return;
    clearTimeout(this.#refreshTimer);
    this.#refreshTimer = setTimeout(() => void this.fetch(), this.#currentInterval);
  }

  /**
   * Fetch data. Called automatically on init and when deps change,
   * or manually via this method.
   */
  async fetch(): Promise<void> {
    if (this.#destroyed || this.#isPaused || this.#fetchInProgress) return;

    this.#fetchInProgress = true;
    this.isLoading = true;
    clearTimeout(this.#loadingHoldTimer);
    const startedAt = performance.now();

    const result = await this.#fetcher();
    if (this.#destroyed) {
      this.#fetchInProgress = false;
      return;
    }

    if (result.isOk) {
      this.data = result.value;
      this.error = null;
      this.#currentInterval = this.#baseInterval; // Reset backoff on success
    } else {
      this.error = result.error;
      // Exponential backoff on error
      this.#currentInterval = Math.min(this.#currentInterval * 2, this.#maxInterval);
    }

    this.#fetchInProgress = false;

    // Hold loading state for minimum duration if specified
    const elapsed = performance.now() - startedAt;
    const remaining = this.#minLoadingMs - elapsed;

    if (remaining > 0) {
      this.#loadingHoldTimer = setTimeout(() => {
        this.isLoading = false;
      }, remaining);
    } else {
      this.isLoading = false;
    }

    this.#scheduleRefresh();
  }

  /** Pause auto-refresh (does not cancel in-flight request) */
  pause(): void {
    this.#isPaused = true;
    clearTimeout(this.#refreshTimer);
  }

  /** Resume auto-refresh and fetch immediately */
  resume(): void {
    if (!this.#isPaused) return;
    this.#isPaused = false;
    void this.fetch();
  }

  /** Clean up timers. Call when component unmounts. */
  destroy(): void {
    this.#destroyed = true;
    this.#clearTimers();
  }

  /** Trigger a fetch, clearing any pending scheduled refresh. */
  trigger(): void {
    clearTimeout(this.#refreshTimer);
    void this.fetch();
  }
}

/**
 * Svelte 5 hook for auto-refreshing data with reactive dependency tracking.
 *
 * @example
 * ```svelte
 * const stats = useAutoRefresh({
 *   fetcher: () => client.getScraperStats(period, term),
 *   deps: () => [period, term],
 *   interval: 5000,
 * });
 *
 * // Access reactive state
 * {stats.data?.count}
 * {#if stats.isLoading}Loading...{/if}
 * {#if stats.error}Error: {stats.error.message}{/if}
 * ```
 */
export function useAutoRefresh<T>(options: AutoRefreshHookOptions<T>): AutoRefreshController<T> {
  const controller = new AutoRefreshController(options);

  // Effect for dependency tracking and initial fetch
  $effect(() => {
    // Track dependencies by calling the deps function
    options.deps?.();

    // Fetch whenever deps change (or on initial run)
    controller.trigger();
  });

  // Cleanup on destroy
  $effect(() => {
    return () => controller.destroy();
  });

  return controller;
}
