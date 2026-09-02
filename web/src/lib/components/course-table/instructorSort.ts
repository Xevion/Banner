import type { SortingState } from "@tanstack/table-core";

/**
 * The instructor header sorts on two different keys, so one click target has to
 * cover both "who teaches it" and "how well they rate". Each click advances
 * through the cycle below; `next` describes what the following click will do.
 */
export interface InstructorSortStep {
  /** Active key, shown beside the header label. Null in the neutral state. */
  readonly key: "name" | "rating" | null;
  readonly indicator: "asc" | "desc" | "none";
  /** Tooltip text. Phrased as the action a click performs, not the state it is in. */
  readonly next: string;
  readonly sorting: SortingState;
}

export const INSTRUCTOR_SORT_CYCLE: readonly InstructorSortStep[] = [
  {
    key: null,
    indicator: "none",
    next: "Click to sort by rating, descending",
    sorting: [],
  },
  {
    key: "rating",
    indicator: "desc",
    next: "Click to sort by rating, ascending",
    sorting: [{ id: "rating", desc: true }],
  },
  {
    key: "rating",
    indicator: "asc",
    next: "Click to sort by name, ascending",
    sorting: [{ id: "rating", desc: false }],
  },
  {
    key: "name",
    indicator: "asc",
    next: "Click to sort by name, descending",
    sorting: [{ id: "instructor", desc: false }],
  },
  {
    key: "name",
    indicator: "desc",
    next: "Click to clear instructor sorting",
    sorting: [{ id: "instructor", desc: true }],
  },
];

/**
 * Which step the given sorting state corresponds to.
 *
 * Sorting by an unrelated column reads as neutral, so clicking the instructor
 * header takes over the sort rather than resuming mid-cycle.
 */
export function instructorSortStep(sorting: SortingState): InstructorSortStep {
  const active = sorting[0];
  if (!active) return INSTRUCTOR_SORT_CYCLE[0];
  const match = INSTRUCTOR_SORT_CYCLE.find(
    (step) => step.sorting[0]?.id === active.id && step.sorting[0]?.desc === active.desc
  );
  return match ?? INSTRUCTOR_SORT_CYCLE[0];
}

/** The sorting state one click past the current one, wrapping back to neutral. */
export function nextInstructorSorting(sorting: SortingState): SortingState {
  const index = INSTRUCTOR_SORT_CYCLE.indexOf(instructorSortStep(sorting));
  return INSTRUCTOR_SORT_CYCLE[(index + 1) % INSTRUCTOR_SORT_CYCLE.length].sorting;
}

/** Header suffix for the active key, e.g. "BY RATING". Null when neutral. */
export function instructorSortLabel(sorting: SortingState): string | null {
  const { key } = instructorSortStep(sorting);
  if (key === null) return null;
  return key === "name" ? "BY NAME" : "BY RATING";
}
