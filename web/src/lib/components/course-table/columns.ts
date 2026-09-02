// columns.ts
import type { CourseResponse } from "$lib/bindings";
import {
  abbreviateInstructor,
  formatMeetingDays,
  formatTimeRange,
  getPrimaryInstructor,
} from "$lib/course";
import type { ColumnDef } from "@tanstack/table-core";
import type { Component } from "svelte";

import CourseCell from "./cells/CourseCell.svelte";
import InstructorCell from "./cells/InstructorCell.svelte";
import SeatsCell from "./cells/SeatsCell.svelte";
import TimeCell from "./cells/TimeCell.svelte";

/**
 * Four stacked columns, each carrying two or three lines. Column IDs double as the
 * backend's `SortColumn` keys, so `course_code` keeps its name despite the wider cell.
 */
export const COLUMN_DEFS: ColumnDef<CourseResponse, unknown>[] = [
  {
    id: "time",
    accessorFn: (row) => {
      if (row.meetingTimes.length === 0) return "";
      const mt = row.meetingTimes[0];
      return `${formatMeetingDays(mt)} ${formatTimeRange(mt.timeRange?.start ?? null, mt.timeRange?.end ?? null)}`;
    },
    header: "Schedule",
    enableSorting: true,
  },
  {
    id: "course_code",
    accessorFn: (row) => `${row.subject} ${row.courseNumber}`,
    header: "Course",
    enableSorting: true,
  },
  {
    id: "instructor",
    accessorFn: (row) => {
      const primary = getPrimaryInstructor(row.instructors, row.primaryInstructorId);
      if (!primary) return "Staff";
      return abbreviateInstructor(primary.displayName);
    },
    header: "Instructor",
    enableSorting: true,
  },
  {
    id: "seats",
    accessorFn: (row) => row.enrollment.max - row.enrollment.current,
    header: "Seats",
    enableSorting: true,
  },
];

/** Column ID to Svelte cell component. Used by the row renderer. */
export const CELL_COMPONENTS: Record<string, Component<{ course: CourseResponse }>> = {
  time: TimeCell,
  course_code: CourseCell,
  instructor: InstructorCell,
  seats: SeatsCell,
};
