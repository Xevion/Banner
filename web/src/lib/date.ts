import { format, formatDistanceToNow } from "date-fns";

/**
 * Utilities for ISO-8601 date string validation and conversion.
 *
 * All DateTime<Utc> fields from Rust are serialized as ISO-8601 strings.
 */

/**
 * Validates if a string is a valid ISO-8601 date string.
 *
 * @param value - The string to validate
 * @returns True if the string is a valid ISO-8601 date
 */
export function isValidISODate(value: string): boolean {
  try {
    const date = new Date(value);
    return !isNaN(date.getTime()) && date.toISOString() === value;
  } catch {
    return false;
  }
}

/**
 * Parses an ISO-8601 date string to a Date object.
 *
 * @param value - The ISO-8601 string to parse
 * @returns Date object, or null if invalid
 */
export function parseISODate(value: string): Date | null {
  try {
    const date = new Date(value);
    if (isNaN(date.getTime())) {
      return null;
    }
    return date;
  } catch {
    return null;
  }
}

/**
 * Asserts that a string is a valid ISO-8601 date, throwing if not.
 *
 * @param value - The string to validate
 * @param fieldName - Name of the field for error messages
 * @throws Error if the string is not a valid ISO-8601 date
 */
export function assertISODate(value: string, fieldName = "date"): void {
  if (!isValidISODate(value)) {
    throw new Error(`Invalid ISO-8601 date for ${fieldName}: ${value}`);
  }
}

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
