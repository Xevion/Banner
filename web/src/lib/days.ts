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
  if (days.length === 0) return "";
  if (days.length === 1) return DAYS[days[0]].short;
  return days.map((d) => DAYS[d].code).join("");
}

/**
 * Comma-separated short names for detail view.
 * Single day -> plural: "Thursdays"
 * Multi-day -> "Mon, Wed, Fri"
 */
export function formatDayList(days: DayOfWeek[]): string {
  if (days.length === 0) return "";
  if (days.length === 1) return DAYS[days[0]].plural;
  return days.map((d) => DAYS[d].short).join(", ");
}

/**
 * Verbose day names for tooltips.
 * "Tuesdays & Thursdays", "Mondays, Wednesdays & Fridays"
 */
export function formatDayVerbose(days: DayOfWeek[]): string {
  const names = days.map((d) => DAYS[d].plural);
  if (names.length === 0) return "";
  if (names.length === 1) return names[0];
  return names.slice(0, -1).join(", ") + " & " + names[names.length - 1];
}

/** Day options for filter UI, derived from canonical data. */
export const DAY_OPTIONS: { label: string; value: DayOfWeek }[] = (
  ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"] as const
).map((d) => ({ label: DAYS[d].code, value: d }));
