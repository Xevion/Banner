import { format, formatDistanceToNow } from "date-fns";

/**
 * Utilities for ISO-8601 date string validation and conversion.
 *
 * All DateTime<Utc> fields from Rust are serialized as ISO-8601 strings.
 */

/**
 * Converts a Date to an ISO-8601 UTC string.
 *
 * @param date - The Date object to convert
 * @returns ISO-8601 string in UTC (e.g., "2024-01-15T10:30:00Z")
 */
export function toISOString(date: Date): string {
  return date.toISOString();
}

/** Returns a relative time string like "3 minutes ago" or "in 2 hours". */
export function formatRelativeDate(date: string | Date): string {
  const d = typeof date === "string" ? new Date(date) : date;
  return formatDistanceToNow(d, { addSuffix: true });
}

/** Returns a full absolute datetime string for tooltip display, e.g. "Jan 29, 2026, 3:45:12 PM". */
export function formatAbsoluteDate(date: string | Date): string {
  const d = typeof date === "string" ? new Date(date) : date;
  return format(d, "MMM d, yyyy, h:mm:ss a");
}

/** Format an ISO-8601 date (YYYY-MM-DD) to "January 20, 2026". */
export function formatDate(dateStr: string): string {
  const [year, month, day] = dateStr.split("-").map(Number);
  if (!year || !month || !day) return dateStr;
  const date = new Date(year, month - 1, day);
  return date.toLocaleDateString("en-US", { year: "numeric", month: "long", day: "numeric" });
}

/** Format an ISO-8601 date (YYYY-MM-DD) as "Aug 26, 2024". */
export function formatDateShort(dateStr: string): string {
  const [year, month, day] = dateStr.split("-").map(Number);
  if (!year || !month || !day) return dateStr;
  const date = new Date(year, month - 1, day);
  return date.toLocaleDateString("en-US", { year: "numeric", month: "short", day: "numeric" });
}
