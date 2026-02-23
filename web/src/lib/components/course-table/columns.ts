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

import CourseCodeCell from "./cells/CourseCodeCell.svelte";
import CrnCell from "./cells/CrnCell.svelte";
import InstructorCell from "./cells/InstructorCell.svelte";
import LocationCell from "./cells/LocationCell.svelte";
import SeatsCell from "./cells/SeatsCell.svelte";
import TimeCell from "./cells/TimeCell.svelte";
import TitleCell from "./cells/TitleCell.svelte";

export const COLUMN_DEFS: ColumnDef<CourseResponse, unknown>[] = [
  {
    id: "crn",
    accessorKey: "crn",
    header: "CRN",
    enableSorting: false,
  },
  {
    id: "course_code",
    accessorFn: (row) => `${row.subject} ${row.courseNumber}`,
    header: "Course",
    enableSorting: true,
  },
  {
    id: "title",
    accessorKey: "title",
    header: "Title",
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
    id: "time",
    accessorFn: (row) => {
      if (row.meetingTimes.length === 0) return "";
      const mt = row.meetingTimes[0];
      return `${formatMeetingDays(mt)} ${formatTimeRange(mt.timeRange?.start ?? null, mt.timeRange?.end ?? null)}`;
    },
    header: "Time",
    enableSorting: true,
  },
  {
    id: "location",
    accessorFn: (row) => row.primaryLocation ?? "",
    header: "Location",
    enableSorting: false,
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
  crn: CrnCell,
  course_code: CourseCodeCell,
  title: TitleCell,
  instructor: InstructorCell,
  time: TimeCell,
  location: LocationCell,
  seats: SeatsCell,
};
