import type { SortKey, SortKeyOption } from "$lib/bindings";

export interface SortTerm {
  key: SortKey;
  desc: boolean;
}

/**
 * Which keys a column's header offers, in the order its clicks walk through.
 *
 * A column is a place to put a control, not the thing being ordered: several
 * keys can share one header, and a key listed nowhere here is still sortable
 * from a menu. Adding a key to a column extends its cycle with no other change.
 */
export const COLUMN_SORTS: Partial<Record<string, SortKey[]>> = {
  days: ["days"],
  time: ["start_time"],
  time_end: ["end_time"],
  duration: ["duration", "weekly_minutes"],
  course_code: ["course_code"],
  title: ["title"],
  instructor: ["instructor_rating", "instructor_name"],
  seats: ["seats_open", "fill_ratio"],
};

/** Every key the backend accepts, kept in sync with `SortKey` in both directions below. */
const SORT_KEYS = [
  "course_code",
  "title",
  "instructor_name",
  "instructor_rating",
  "start_time",
  "end_time",
  "duration",
  "weekly_minutes",
  "days",
  "seats_open",
  "fill_ratio",
  "wait_count",
] as const satisfies readonly SortKey[];

type ListedSortKey = (typeof SORT_KEYS)[number];

// Compile-time assertion, mirroring the one in filters.ts: SortKey must be
// assignable to ListedSortKey, so a key the backend adds and this list omits
// fails the build rather than silently passing runtime validation.
const _sortKeyExhaustiveCheck: ListedSortKey = {} as SortKey;
void _sortKeyExhaustiveCheck;

const VALID_SORT_KEYS: ReadonlySet<SortKey> = new Set(SORT_KEYS);

/** Wire format: comma separated, `-` prefixed for descending. Mirrors the backend. */
export function formatSort(terms: SortTerm[]): string {
  return terms.map((term) => `${term.desc ? "-" : ""}${term.key}`).join(",");
}

/** Drops unrecognized or repeated keys rather than surfacing a parse error. */
export function parseSort(raw: string | null): SortTerm[] {
  if (!raw) return [];
  const seen = new Set<SortKey>();
  const terms: SortTerm[] = [];
  for (const part of raw
    .split(",")
    .map((p) => p.trim())
    .filter(Boolean)) {
    const desc = part.startsWith("-");
    const key = (desc ? part.slice(1) : part) as SortKey;
    if (!VALID_SORT_KEYS.has(key) || seen.has(key)) continue;
    seen.add(key);
    terms.push({ key, desc });
  }
  return terms;
}

/**
 * The states a header walks through: each of its keys ascending then descending,
 * then back to unsorted.
 *
 * The instructor header's rating/name cycle is just this rule applied to a column
 * with two keys, rather than a mechanism of its own.
 */
function cycleFor(keys: SortKey[]): (SortTerm | null)[] {
  const steps: (SortTerm | null)[] = [];
  for (const key of keys) {
    steps.push({ key, desc: false });
    steps.push({ key, desc: true });
  }
  steps.push(null);
  return steps;
}

export interface HeaderSortStep {
  /** The term this header currently contributes, if any. */
  active: SortTerm | null;
  /** Shown beside the label when a column offers more than one key. */
  suffix: string | null;
  indicator: "asc" | "desc" | "none";
  /** What clicking does next, phrased as an action. */
  title: string;
  next: SortTerm | null;
}

/**
 * Resolve a header's current state and its next click against the active sort.
 *
 * Entering from an unrelated sort starts the cycle over rather than resuming
 * where this header last left it.
 */
export function headerSortStep(
  columnId: string,
  terms: SortTerm[],
  labels: Map<SortKey, SortKeyOption>
): HeaderSortStep | null {
  const keys = COLUMN_SORTS[columnId];
  if (!keys || keys.length === 0) return null;

  const steps = cycleFor(keys);
  const lead = terms[0] ?? null;
  const activeIndex =
    lead === null
      ? -1
      : steps.findIndex(
          (step) => step !== null && step.key === lead.key && step.desc === lead.desc
        );

  const active = activeIndex === -1 ? null : steps[activeIndex]!;
  const next = steps[(activeIndex + 1) % steps.length];

  const describe = (term: SortTerm | null) => {
    if (!term) return "Click to clear sorting";
    const option = labels.get(term.key);
    const label = option ? (term.desc ? option.descLabel : option.ascLabel) : term.key;
    return `Click to sort: ${label.toLowerCase()}`;
  };

  return {
    active,
    // Only worth naming the key when the column offers a choice of them.
    suffix: active && keys.length > 1 ? active.key.replace(/_/g, " ").toUpperCase() : null,
    indicator: active ? (active.desc ? "desc" : "asc") : "none",
    title: describe(next),
    next,
  };
}

/** Replace the whole sort with this header's next state. */
export function applyHeaderSort(next: SortTerm | null): SortTerm[] {
  return next ? [next] : [];
}
