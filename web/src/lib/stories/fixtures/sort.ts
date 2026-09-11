import type { SortKeyOption } from "$lib/bindings";

/**
 * The key catalogue as `/api/search-options` serves it, which Storybook has no
 * backend to ask for. Mirrors the labels in `src/data/courses/sort.rs`, so a
 * header tooltip reads in a story exactly as it does against the real API.
 */
export const sortCatalog: SortKeyOption[] = [
  { key: "course_code", ascLabel: "A to Z", descLabel: "Z to A" },
  { key: "title", ascLabel: "A to Z", descLabel: "Z to A" },
  { key: "instructor_name", ascLabel: "Name, A to Z", descLabel: "Name, Z to A" },
  { key: "instructor_rating", ascLabel: "Lowest rated", descLabel: "Highest rated" },
  { key: "start_time", ascLabel: "Earliest first", descLabel: "Latest first" },
  { key: "end_time", ascLabel: "Ends earliest", descLabel: "Ends latest" },
  { key: "duration", ascLabel: "Shortest first", descLabel: "Longest first" },
  { key: "weekly_minutes", ascLabel: "Least time per week", descLabel: "Most time per week" },
  { key: "days", ascLabel: "Earliest weekday first", descLabel: "Latest weekday first" },
  { key: "seats_open", ascLabel: "Nearly full", descLabel: "Most seats open" },
  { key: "fill_ratio", ascLabel: "Emptiest first", descLabel: "Fullest first" },
  { key: "wait_count", ascLabel: "Shortest waitlist", descLabel: "Longest waitlist" },
];
