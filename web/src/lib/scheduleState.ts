import type { CourseResponse } from "$lib/bindings";

/**
 * Why a section has no meeting time, which the meeting rows alone cannot say.
 *
 * A missing time is almost never "not announced yet": independent study and
 * asynchronous online sections are never scheduled by design, and together they
 * are most of a term's catalog.
 */
export type ScheduleState = "timed" | "async" | "arranged" | "unscheduled";

export interface ScheduleCopy {
  /** Short badge in the days column, sized to the day-chip strip. */
  badge: string;
  /** Stands in for the time, in place of a placeholder glyph. */
  phrase: string;
  /**
   * Tooltip explaining what the state means for a student.
   *
   * Broken into short explicit lines, not left to wrap: the tooltip is
   * shrink-to-fit under a max-width, so wrapped prose pins it to the full cap
   * and leaves a gutter, while pre-set lines make the box hug the longest one.
   */
  detail: string;
}

export function scheduleState(course: CourseResponse): ScheduleState {
  if (course.meetingTimes.some((mt) => mt.timeRange !== null)) return "timed";
  if (course.isAsyncOnline) return "async";
  if (course.instructionalMethod?.type === "Independent") return "arranged";
  return "unscheduled";
}

export function scheduleCopy(state: ScheduleState): ScheduleCopy | null {
  if (state === "timed") return null;

  if (state === "async") {
    return {
      badge: "ASYNC",
      phrase: "Self-paced",
      detail: "Asynchronous online.\nNo scheduled meetings.",
    };
  }
  if (state === "arranged") {
    return {
      badge: "ARRANGED",
      phrase: "With instructor",
      detail: "Independent study or research.\nTimes arranged with your instructor.",
    };
  }
  return {
    badge: "NO TIMES",
    phrase: "Not scheduled",
    detail: "No schedule was ever published.\nUsually a dormant listing.",
  };
}

/**
 * An unassigned instructor means different things by section type: an unclaimed
 * independent-study shell is waiting for whoever supervises you, while a
 * scheduled section is simply missing an assignment.
 */
export function unassignedInstructorDetail(course: CourseResponse): string {
  return course.instructionalMethod?.type === "Independent"
    ? "Listed per faculty member.\nSet when you register."
    : "No instructor assigned yet.";
}
