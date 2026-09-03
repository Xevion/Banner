import type { SortKey, SortKeyOption } from "$lib/bindings";
import { MAX_SORT_TERMS, type SortTerm } from "$lib/sort";

/** Which way round a key orders, for callers that would rather not read a boolean. */
export type SortDirection = "asc" | "desc";

/** Which way a term moves through the priority list. */
export type MoveDirection = "up" | "down";

export interface UseSortOptions {
  /** The key catalogue with its labels, read lazily so it can arrive with the page data. */
  catalog: () => SortKeyOption[];
  /** Terms the page starts with, highest priority first. */
  initial?: SortTerm[];
  /** Runs after every change made through this controller. `sync` deliberately skips it. */
  onChange?: (terms: SortTerm[]) => void;
  /** Cap on active terms. Defaults to what the backend will read. */
  maxTerms?: number;
}

/**
 * Owns the ordered sort the search sends to the backend.
 *
 * Terms are a priority list: the first orders the results, the second breaks its
 * ties, and so on up to `maxTerms`. Nothing here truncates silently: `append`
 * refuses at the cap so a menu can disable the control rather than swallow a click.
 */
export class SortController {
  /** Cap on active terms, mirroring the backend. */
  readonly maxTerms: number;

  #terms: SortTerm[] = $state([]);
  readonly #catalog: () => SortKeyOption[];
  readonly #onChange: ((terms: SortTerm[]) => void) | undefined;

  constructor(options: UseSortOptions) {
    this.#catalog = options.catalog;
    this.#onChange = options.onChange;
    this.maxTerms = options.maxTerms ?? MAX_SORT_TERMS;
    this.#terms = [...(options.initial ?? [])];
  }

  /** Active terms, highest priority first. */
  get terms(): readonly SortTerm[] {
    return this.#terms;
  }

  /** Every key the backend offers, in the order it offers them. */
  get catalog(): readonly SortKeyOption[] {
    return this.#catalog();
  }

  /** Catalogue entries with no active term, still in catalogue order. */
  get available(): readonly SortKeyOption[] {
    return this.#catalog().filter((option) => !this.isActive(option.key));
  }

  get isEmpty(): boolean {
    return this.#terms.length === 0;
  }

  /** At the cap: `append` refuses until something is removed. */
  get isFull(): boolean {
    return this.#terms.length >= this.maxTerms;
  }

  isActive = (key: SortKey): boolean => this.indexOf(key) !== -1;

  /** Priority position of a key, or -1 when it is not sorting. */
  indexOf = (key: SortKey): number => this.#terms.findIndex((term) => term.key === key);

  /** Which way a key orders right now, or null when it is not sorting. */
  directionOf = (key: SortKey): SortDirection | null => {
    const term = this.#terms.find((candidate) => candidate.key === key);
    if (!term) return null;
    return term.desc ? "desc" : "asc";
  };

  /** How a key reads one way round, from the injected catalogue. */
  label = (key: SortKey, desc: boolean): string => {
    const option = this.#catalog().find((candidate) => candidate.key === key);
    if (!option) return key;
    return desc ? option.descLabel : option.ascLabel;
  };

  /** How an active term reads as it currently stands. */
  labelOf = (term: SortTerm): string => this.label(term.key, term.desc);

  /**
   * Apply a header's next state.
   *
   * The policy lives here: a header click replaces the whole sort rather than
   * extending it, so a table whose headers should append instead changes once.
   */
  applyHeaderClick = (next: SortTerm | null): void => {
    if (next) this.replace(next);
    else this.clear();
  };

  /** Make this the only term, dropping everything else. */
  replace = (term: SortTerm): void => {
    this.#set([{ key: term.key, desc: term.desc }]);
  };

  clear = (): void => {
    this.#set([]);
  };

  /** Add as the lowest-priority tiebreaker. Refuses at the cap or on an active key. */
  append = (term: SortTerm): boolean => {
    if (this.isFull || this.isActive(term.key)) return false;
    this.#set([...this.#terms, { key: term.key, desc: term.desc }]);
    return true;
  };

  remove = (key: SortKey): boolean => {
    if (!this.isActive(key)) return false;
    this.#set(this.#terms.filter((term) => term.key !== key));
    return true;
  };

  /** Point an active key the other way round, leaving its priority alone. */
  toggleDirection = (key: SortKey): boolean => {
    const direction = this.directionOf(key);
    if (direction === null) return false;
    return this.setDirection(key, direction === "asc");
  };

  setDirection = (key: SortKey, desc: boolean): boolean => {
    const index = this.indexOf(key);
    if (index === -1 || this.#terms[index].desc === desc) return false;
    this.#set(this.#terms.map((term) => (term.key === key ? { key, desc } : term)));
    return true;
  };

  /** Shift a key one place up or down the priority list. */
  move = (key: SortKey, direction: MoveDirection): boolean => {
    const index = this.indexOf(key);
    if (index === -1) return false;
    const target = direction === "up" ? index - 1 : index + 1;
    if (target < 0 || target >= this.#terms.length) return false;
    const next = [...this.#terms];
    const [term] = next.splice(index, 1);
    next.splice(target, 0, term);
    this.#set(next);
    return true;
  };

  /**
   * Adopt terms decided elsewhere, such as the URL the browser navigated to.
   * Silent by design: whatever set them already knows.
   */
  sync = (terms: readonly SortTerm[]): void => {
    this.#terms = [...terms];
  };

  #set(next: SortTerm[]): void {
    this.#terms = next;
    this.#onChange?.(next);
  }
}
