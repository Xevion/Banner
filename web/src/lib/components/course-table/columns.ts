// columns.ts

import type { ColumnDef } from "@tanstack/table-core";
import type { Component } from "svelte";
import type { CourseResponse } from "$lib/bindings";
import { abbreviateInstructor, formatMeetingDays, getPrimaryInstructor } from "$lib/course";
import { meetingDurationMinutes } from "$lib/schedule";

import CourseCell from "./cells/CourseCell.svelte";
import DaysCell from "./cells/DaysCell.svelte";
import DurationCell from "./cells/DurationCell.svelte";
import InstructorCell from "./cells/InstructorCell.svelte";
import SeatsCell from "./cells/SeatsCell.svelte";
import TimeCell from "./cells/TimeCell.svelte";
import TimeEndCell from "./cells/TimeEndCell.svelte";
import TitleCell from "./cells/TitleCell.svelte";
import { COLUMN_IDS, type ColumnId } from "./column-ids";

export type { ColumnId };

interface ColumnSpec {
  /** Header text, and the label shown in the column-visibility toggles. */
  label: string;
  accessorFn: (row: CourseResponse) => unknown;
  /** Fixed track width in px; the flex column's entry is only the floor it may shrink to. */
  width: number;
  cell: Component<{ course: CourseResponse }>;
}

/**
 * Everything about one course table column, keyed by `ColumnId` so a missing or
 * misspelled entry is a compile error. `COLUMN_DEFS` and the visibility list
 * below are both derived from this rather than re-declaring the column set.
 *
 * Sorting is not declared here: `COLUMN_SORTS` in `sort.ts` maps a column to the
 * sort keys its header cycles through, since that's a sorting concern, not a
 * rendering one, and not every column is sortable.
 */
export const COLUMNS = {
  days: {
    label: "Days",
    accessorFn: (row) => (row.meetingTimes[0] ? formatMeetingDays(row.meetingTimes[0]) : ""),
    // 110px of day chips (7 x 14 + 6 x 2) plus the cell's own padding. Anything
    // narrower makes flex shrink the chips off their own grid.
    width: 126,
    cell: DaysCell,
  },
  time: {
    label: "Start Time",
    accessorFn: (row) => row.meetingTimes[0]?.timeRange?.start ?? "",
    // Fits the "START TIME" header with its sort arrow, and the untimed phrase
    // that stands in for a clock value.
    width: 112,
    cell: TimeCell,
  },
  time_end: {
    label: "End Time",
    accessorFn: (row) => row.meetingTimes[0]?.timeRange?.end ?? "",
    // Off by default. Shown, it sits flush against the start and reads as one
    // range, but each half aligns in its own track instead of drifting.
    width: 112,
    cell: TimeEndCell,
  },
  duration: {
    label: "Duration",
    accessorFn: (row) => {
      const mt = row.meetingTimes[0];
      return mt ? (meetingDurationMinutes(mt) ?? 0) : 0;
    },
    // Its own track so durations align down the page; a sub-column inside the
    // time cell sizes per row and drifts.
    width: 84,
    cell: DurationCell,
  },
  course_code: {
    label: "Course",
    accessorFn: (row) => `${row.subject} ${row.courseNumber}`,
    width: 108,
    cell: CourseCell,
  },
  title: {
    label: "Title",
    accessorFn: (row) => row.title,
    // The floor, not a fixed width: title is the flex track, so it gives up its
    // slack before the table scrolls. Truncation beats a horizontal scrollbar.
    width: 200,
    cell: TitleCell,
  },
  instructor: {
    label: "Instructor",
    accessorFn: (row) => {
      const primary = getPrimaryInstructor(row.instructors, row.primaryInstructorId);
      return primary ? abbreviateInstructor(primary.displayName) : "";
    },
    width: 212,
    cell: InstructorCell,
  },
  seats: {
    label: "Seats",
    accessorFn: (row) => row.enrollment.max - row.enrollment.current,
    width: 132,
    cell: SeatsCell,
  },
} satisfies Record<ColumnId, ColumnSpec>;

export const COLUMN_DEFS = COLUMN_IDS.map((id) => ({
  id,
  accessorFn: COLUMNS[id].accessorFn,
  header: COLUMNS[id].label,
  enableSorting: false,
})) satisfies ColumnDef<CourseResponse, unknown>[];

/**
 * The one track left unsized in the colgroup. Surplus width collects here rather
 * than being shared across every column, and a shortfall is taken from here
 * first. Its width above is the floor before the table scrolls.
 */
export const FLEX_COLUMN: ColumnId = "title";

/** Narrowest the table can be before the wrapper scrolls, for the visible set. */
export function tableMinWidth(visibleIds: ColumnId[]): number {
  return visibleIds.reduce((total, id) => total + COLUMNS[id].width, 0);
}
