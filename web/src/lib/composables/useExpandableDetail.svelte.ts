/**
 * One-at-a-time expandable row detail: tracks which row is open, lazily fetches
 * its detail payload, and closes on Escape.
 */

import type { ApiErrorClass } from "$lib/api";
import type Result from "true-myth/result";

export interface UseExpandableDetailOptions<T> {
  /** Fetches the detail payload for a row id. */
  fetcher: (id: number) => Promise<Result<T, ApiErrorClass>>;
  /** Runs before each fetch, to reset page state scoped to the open row. */
  beforeLoad?: () => void;
  /** Runs whenever the open row closes. */
  onCollapse?: () => void;
}

export function useExpandableDetail<T>(options: UseExpandableDetailOptions<T>) {
  let expandedId = $state<number | null>(null);
  let detail = $state<T | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);

  /** Fetches (or refetches) the detail for a row without changing which row is open. */
  async function load(id: number) {
    options.beforeLoad?.();
    loading = true;
    error = null;
    detail = null;
    const result = await options.fetcher(id);
    if (result.isErr) {
      error = result.error.message;
    } else {
      detail = result.value;
    }
    loading = false;
  }

  function collapse() {
    expandedId = null;
    detail = null;
    options.onCollapse?.();
  }

  async function toggle(id: number) {
    if (expandedId === id) {
      collapse();
      return;
    }
    expandedId = id;
    await load(id);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && expandedId !== null) collapse();
  }

  return {
    get expandedId() {
      return expandedId;
    },
    get detail() {
      return detail;
    },
    set detail(value: T | null) {
      detail = value;
    },
    get loading() {
      return loading;
    },
    get error() {
      return error;
    },
    set error(value: string | null) {
      error = value;
    },
    load,
    toggle,
    collapse,
    handleKeydown,
  };
}
