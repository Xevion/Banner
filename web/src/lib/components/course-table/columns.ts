// columns.ts
import type { CourseResponse } from "$lib/bindings";
import { abbreviateInstructor, formatMeetingDays, getPrimaryInstructor } from "$lib/course";
import { meetingDurationMinutes } from "$lib/schedule";
import type { ColumnDef } from "@tanstack/table-core";
import type { Component } from "svelte";

import CourseCell from "./cells/CourseCell.svelte";
import DaysCell from "./cells/DaysCell.svelte";
import DurationCell from "./cells/DurationCell.svelte";
import InstructorCell from "./cells/InstructorCell.svelte";
import SeatsCell from "./cells/SeatsCell.svelte";
import TimeCell from "./cells/TimeCell.svelte";
import TimeEndCell from "./cells/TimeEndCell.svelte";
import TitleCell from "./cells/TitleCell.svelte";

/**
 * One line per section: each column carries a single fact, sized by `COLUMN_WIDTHS`.
 *
 * Sorting is not declared here. A column offers whichever sort keys `COLUMN_SORTS`
 * maps to it, and the header cycle drives the server, so TanStack's own per-column
 * toggle stays off throughout.
 */
export const COLUMN_DEFS: ColumnDef<CourseResponse, unknown>[] = [
  {
    id: "days",
    accessorFn: (row) => (row.meetingTimes[0] ? formatMeetingDays(row.meetingTimes[0]) : ""),
    header: "Days",
    enableSorting: false,
  },
  {
    id: "time",
    accessorFn: (row) => row.meetingTimes[0]?.timeRange?.start ?? "",
    header: "Start Time",
    enableSorting: false,
  },
  {
    // Off by default. Shown, it sits flush against the start and reads as one
    // range, but each half aligns in its own track instead of drifting.
    id: "time_end",
    accessorFn: (row) => row.meetingTimes[0]?.timeRange?.end ?? "",
    header: "End Time",
    enableSorting: false,
  },
  {
    // Its own track so durations align down the page; a sub-column inside the
    // time cell sizes per row and drifts.
    id: "duration",
    accessorFn: (row) => {
      const mt = row.meetingTimes[0];
      return mt ? (meetingDurationMinutes(mt) ?? 0) : 0;
    },
    header: "Duration",
    enableSorting: false,
  },
  {
    id: "course_code",
    accessorFn: (row) => `${row.subject} ${row.courseNumber}`,
    header: "Course",
    enableSorting: false,
  },
  {
    id: "title",
    accessorFn: (row) => row.title,
    header: "Title",
    enableSorting: false,
  },
  {
    id: "instructor",
    accessorFn: (row) => {
      const primary = getPrimaryInstructor(row.instructors, row.primaryInstructorId);
      return primary ? abbreviateInstructor(primary.displayName) : "";
    },
    header: "Instructor",
    enableSorting: false,
  },
  {
    id: "seats",
    accessorFn: (row) => row.enrollment.max - row.enrollment.current,
    header: "Seats",
    enableSorting: false,
  },
];

/**
 * Track widths in px, so every row lands on the same grid. Every column but the
 * flex one is fixed here; the flex column's entry is the floor it may shrink to.
 */
export const COLUMN_WIDTHS: Record<string, number> = {
  // 110px of day chips (7 x 14 + 6 x 2) plus the cell's own padding. Anything
  // narrower makes flex shrink the chips off their own grid.
  days: 126,
  // Fits the "START TIME" header with its sort arrow, and the untimed phrase
  // that stands in for a clock value.
  time: 112,
  time_end: 112,
  duration: 84,
  course_code: 108,
  // The floor, not a fixed width: title is the flex track, so it gives up its
  // slack before the table scrolls. Truncation beats a horizontal scrollbar.
  title: 200,
  instructor: 212,
  seats: 132,
};

/**
 * The one track left unsized in the colgroup. Surplus width collects here rather
 * than being shared across every column, and a shortfall is taken from here
 * first. Its `COLUMN_WIDTHS` entry is the floor before the table scrolls.
 */
export const FLEX_COLUMN = "title";

/** Narrowest the table can be before the wrapper scrolls, for the visible set. */
export function tableMinWidth(visibleIds: string[]): number {
  return visibleIds.reduce((total, id) => total + (COLUMN_WIDTHS[id] ?? 0), 0);
}

/** Column ID to Svelte cell component. Used by the row renderer. */
export const CELL_COMPONENTS: Record<string, Component<{ course: CourseResponse }>> = {
  days: DaysCell,
  time: TimeCell,
  time_end: TimeEndCell,
  duration: DurationCell,
  course_code: CourseCell,
  title: TitleCell,
  instructor: InstructorCell,
  seats: SeatsCell,
};
