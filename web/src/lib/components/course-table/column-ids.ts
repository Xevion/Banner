/**
 * The full set of course table columns, in display order. The single source
 * every column ID derives from, kept dependency-free so pure modules (sort.ts,
 * universal load functions) don't pull in the table's Svelte cell components.
 */
export const COLUMN_IDS = [
  "days",
  "time",
  "time_end",
  "duration",
  "course_code",
  "title",
  "instructor",
  "seats",
] as const;

export type ColumnId = (typeof COLUMN_IDS)[number];

const COLUMN_ID_SET: ReadonlySet<string> = new Set(COLUMN_IDS);

/** Narrows a loosely-typed id (e.g. a TanStack header id) to a known `ColumnId`. */
export function isColumnId(id: string): id is ColumnId {
  return COLUMN_ID_SET.has(id);
}
