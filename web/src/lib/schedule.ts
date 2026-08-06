import type { DayOfWeek, DbMeetingTime } from "$lib/bindings";

/** Day order of the week strip, Monday-first. */
const SCHEDULE_DAYS: DayOfWeek[] = [
  "monday",
  "tuesday",
  "wednesday",
  "thursday",
  "friday",
  "saturday",
  "sunday",
];

/** In-cell key letters for the week strip, parallel to `SCHEDULE_DAYS`. */
export const SCHEDULE_DAY_LABELS = ["M", "T", "W", "Th", "F", "Sa", "Su"];

/** The strip spans 7 AM to 10 PM, divided into five three-hour segments. */
const TRACK_START_MINUTES = 7 * 60;
const TRACK_END_MINUTES = 22 * 60;
const TRACK_SPAN_MINUTES = TRACK_END_MINUTES - TRACK_START_MINUTES;

/** Meetings outside the window still get a sliver so the row never reads as untimed. */
const MIN_SPAN_PERCENT = 1.5;

/** A blue run on the day track, as percentages of the 7am-10pm window. */
export interface TrackSpan {
  left: number;
  width: number;
}

/** Minutes since midnight from an ISO "HH:MM:SS" time, or null if unparseable. */
export function parseTimeMinutes(time: string | null): number | null {
  if (!time) return null;
  const parts = time.split(":");
  if (parts.length < 2) return null;
  const hours = Number(parts[0]);
  const minutes = Number(parts[1]);
  if (!Number.isFinite(hours) || !Number.isFinite(minutes)) return null;
  return hours * 60 + minutes;
}

/** Human duration: "50 min", "1h", "1h 15m". */
export function formatDuration(minutes: number): string {
  if (minutes < 60) return `${minutes} min`;
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  return rest === 0 ? `${hours}h` : `${hours}h ${rest}m`;
}

/** Length of a meeting in minutes, or null when the time range is TBA. */
export function meetingDurationMinutes(mt: DbMeetingTime): number | null {
  const start = parseTimeMinutes(mt.timeRange?.start ?? null);
  const end = parseTimeMinutes(mt.timeRange?.end ?? null);
  if (start === null || end === null) return null;
  return end - start;
}

/**
 * Which days of the week the course meets, unioned across every meeting time.
 * Parallel to `SCHEDULE_DAYS`, so a lecture on MWF plus a lab on T lights up four cells.
 */
export function meetingDayFlags(meetingTimes: DbMeetingTime[]): boolean[] {
  const active = new Set<DayOfWeek>();
  for (const mt of meetingTimes) {
    for (const day of mt.days) active.add(day);
  }
  return SCHEDULE_DAYS.map((day) => active.has(day));
}

/**
 * Where each meeting sits within the 7am-10pm window, as left/width percentages.
 *
 * Meetings are clamped to the window, sorted by start, and merged when they
 * overlap so the track never paints the same run twice.
 */
export function meetingTrackSpans(meetingTimes: DbMeetingTime[]): TrackSpan[] {
  const ranges: [number, number][] = [];

  for (const mt of meetingTimes) {
    const start = parseTimeMinutes(mt.timeRange?.start ?? null);
    const end = parseTimeMinutes(mt.timeRange?.end ?? null);
    if (start === null || end === null || end < start) continue;

    const left = ((clampToWindow(start) - TRACK_START_MINUTES) / TRACK_SPAN_MINUTES) * 100;
    const right = ((clampToWindow(end) - TRACK_START_MINUTES) / TRACK_SPAN_MINUTES) * 100;
    const width = Math.max(right - left, MIN_SPAN_PERCENT);
    ranges.push([Math.min(left, 100 - width), width]);
  }

  ranges.sort((a, b) => a[0] - b[0]);

  const merged: TrackSpan[] = [];
  for (const [left, width] of ranges) {
    const previous = merged[merged.length - 1];
    if (previous && left <= previous.left + previous.width) {
      previous.width = Math.max(previous.width, left + width - previous.left);
    } else {
      merged.push({ left, width });
    }
  }
  return merged;
}

function clampToWindow(minutes: number): number {
  return Math.min(Math.max(minutes, TRACK_START_MINUTES), TRACK_END_MINUTES);
}
