/**
 * Typeahead suggestion fetching, shared by the boxes that ask the server as the
 * user types.
 *
 * Beyond the debouncing and stale-response handling of a plain query, a
 * suggestion box has to answer to a query that is still being typed: a query
 * too short to mean anything is not asked at all, results stop applying the
 * moment the text they matched is gone, and "no answer yet" has to stay
 * distinguishable from "the answer is nothing".
 */

import type { ApiErrorClass } from "$lib/api";
import type Result from "true-myth/result";

export interface SuggestionQueryOptions<T> {
  /** Asks the server about a query already known to be long enough. */
  fetcher: (query: string) => Promise<Result<T, ApiErrorClass>>;
  /** The value meaning "nothing to show", used before and between queries. */
  empty: T;
  debounce?: number;
  minLength?: number;
}

const DEFAULT_DEBOUNCE_MS = 250;
const DEFAULT_MIN_LENGTH = 2;

export class SuggestionQuery<T> {
  /** The raw text in the box, including whitespace the user typed. */
  text = $state("");
  data = $state<T>(undefined as T);
  error = $state<string | null>(null);
  loading = $state(false);
  /** Whether a response has arrived for the query currently in the box. */
  settled = $state(false);
  open = $state(false);

  readonly #fetcher: (query: string) => Promise<Result<T, ApiErrorClass>>;
  readonly #empty: T;
  readonly #debounceMs: number;
  readonly #minLength: number;

  #timer: ReturnType<typeof setTimeout> | undefined;
  #fetchId = 0;
  #destroyed = false;

  constructor(options: SuggestionQueryOptions<T>) {
    this.#fetcher = options.fetcher;
    this.#empty = options.empty;
    this.#debounceMs = options.debounce ?? DEFAULT_DEBOUNCE_MS;
    this.#minLength = options.minLength ?? DEFAULT_MIN_LENGTH;
    this.data = options.empty;
  }

  /** The query as the server sees it. */
  get trimmed(): string {
    return this.text.trim();
  }

  get isLongEnough(): boolean {
    return this.trimmed.length >= this.#minLength;
  }

  setQuery(next: string): void {
    if (this.#destroyed) return;
    this.text = next;
    clearTimeout(this.#timer);
    // Any answer still in flight was to a query that is no longer in the box.
    this.#fetchId++;

    if (!this.isLongEnough) {
      this.#clearResults();
      this.loading = false;
      this.open = false;
      return;
    }

    this.#clearResults();
    this.open = true;
    this.loading = true;
    this.#timer = setTimeout(() => void this.#fetch(), this.#debounceMs);
  }

  /** Empties the box after its suggestion has been acted on. */
  reset(): void {
    clearTimeout(this.#timer);
    this.#fetchId++;
    this.text = "";
    this.#clearResults();
    this.loading = false;
    this.open = false;
  }

  destroy(): void {
    this.#destroyed = true;
    clearTimeout(this.#timer);
    this.#timer = undefined;
  }

  #clearResults(): void {
    this.data = this.#empty;
    this.error = null;
    this.settled = false;
  }

  async #fetch(): Promise<void> {
    const id = this.#fetchId;
    const result = await this.#fetcher(this.trimmed);
    if (this.#destroyed || id !== this.#fetchId) return;

    result.match({
      Ok: (data) => {
        this.data = data;
        this.error = null;
      },
      Err: (e) => {
        this.data = this.#empty;
        this.error = e.message ?? "Failed to fetch suggestions";
      },
    });

    this.loading = false;
    this.settled = true;
  }
}

/** Ties a query's lifetime to the component that owns it. */
export function useSuggestions<T>(options: SuggestionQueryOptions<T>): SuggestionQuery<T> {
  const query = new SuggestionQuery(options);
  $effect(() => () => query.destroy());
  return query;
}
