import type { DayOfWeek } from "$lib/bindings";

/** Canonical day-of-week display data. Single source of truth for all formatting tiers. */
const DAYS: Record<DayOfWeek, { code: string; short: string; plural: string }> = {
  monday: { code: "M", short: "Mon", plural: "Mondays" },
  tuesday: { code: "T", short: "Tue", plural: "Tuesdays" },
  wednesday: { code: "W", short: "Wed", plural: "Wednesdays" },
  thursday: { code: "Th", short: "Thu", plural: "Thursdays" },
  friday: { code: "F", short: "Fri", plural: "Fridays" },
  saturday: { code: "Sa", short: "Sat", plural: "Saturdays" },
  sunday: { code: "Su", short: "Sun", plural: "Sundays" },
};

/** Single-char (or two-char) code: "M", "Th", "Sa" */
export function dayCode(d: DayOfWeek): string {
  return DAYS[d].code;
}

/** Three-letter abbreviation: "Mon", "Thu" */
export function dayShort(d: DayOfWeek): string {
  return DAYS[d].short;
}

/** Plural name: "Mondays", "Thursdays" */
export function dayPlural(d: DayOfWeek): string {
  return DAYS[d].plural;
}

/**
 * Compact concatenated codes for table cells.
 * Single day -> 3-letter: "Mon", "Thu"
 * Multi-day -> concatenated codes: "MWF", "TTh"
 */
export function formatDayCodes(days: DayOfWeek[]): string {
  const [first] = days;
  if (first === undefined) return "";
  if (days.length === 1) return DAYS[first].short;
  return days.map((d) => DAYS[d].code).join("");
}

/**
 * Comma-separated short names for detail view.
 * Single day -> plural: "Thursdays"
 * Multi-day -> "Mon, Wed, Fri"
 */
export function formatDayList(days: DayOfWeek[]): string {
  const [first] = days;
  if (first === undefined) return "";
  if (days.length === 1) return DAYS[first].plural;
  return days.map((d) => DAYS[d].short).join(", ");
}

/**
 * Verbose day names for tooltips.
 * "Tuesdays & Thursdays", "Mondays, Wednesdays & Fridays"
 */
export function formatDayVerbose(days: DayOfWeek[]): string {
  const names = days.map((d) => DAYS[d].plural);
  const last = names.pop();
  if (last === undefined) return "";
  if (names.length === 0) return last;
  return `${names.join(", ")} & ${last}`;
}

/** Day options for filter UI, derived from canonical data. */
export const DAY_OPTIONS: { label: string; value: DayOfWeek }[] = (
  ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"] as const
).map((d) => ({ label: DAYS[d].code, value: d }));
