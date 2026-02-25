import type {
  Campus,
  CourseResponse,
  DbMeetingTime,
  InstructionalMethod,
  InstructorResponse,
} from "$lib/bindings";
import { formatDateShort } from "$lib/date";
import { formatDayCodes, formatDayList, formatDayVerbose } from "$lib/days";

/** Convert ISO time string "08:30:00" to "8:30 AM" */
export function formatISOTime(time: string | null): string {
  if (!time) return "TBA";
  // ISO format: "HH:MM:SS"
  const parts = time.split(":");
  if (parts.length < 2) return "TBA";
  const hours = parseInt(parts[0], 10);
  const minutes = parts[1];
  const period = hours >= 12 ? "PM" : "AM";
  const display = hours > 12 ? hours - 12 : hours === 0 ? 12 : hours;
  return `${display}:${minutes} ${period}`;
}

export function formatMeetingDays(mt: DbMeetingTime): string {
  return formatDayCodes(mt.days);
}

export function formatMeetingDaysLong(mt: DbMeetingTime): string {
  return formatDayList(mt.days);
}

/**
 * Format a time range with smart AM/PM elision.
 *
 * Same period:  "9:00–9:50 AM"
 * Cross-period: "11:30 AM–12:20 PM"
 * Missing:      "TBA"
 */
export function formatTimeRange(begin: string | null, end: string | null): string {
  if (!begin || !end) return "TBA";

  const bParts = begin.split(":");
  const eParts = end.split(":");
  if (bParts.length < 2 || eParts.length < 2) return "TBA";

  const bHours = parseInt(bParts[0], 10);
  const eHours = parseInt(eParts[0], 10);
  const bPeriod = bHours >= 12 ? "PM" : "AM";
  const ePeriod = eHours >= 12 ? "PM" : "AM";

  const bDisplay = bHours > 12 ? bHours - 12 : bHours === 0 ? 12 : bHours;
  const eDisplay = eHours > 12 ? eHours - 12 : eHours === 0 ? 12 : eHours;

  const endStr = `${eDisplay}:${eParts[1]} ${ePeriod}`;
  if (bPeriod === ePeriod) {
    return `${bDisplay}:${bParts[1]}–${endStr}`;
  }
  return `${bDisplay}:${bParts[1]} ${bPeriod}–${endStr}`;
}

/**
 * Progressively abbreviate an instructor name to fit within a character budget.
 *
 * Tries each level until the result fits `maxLen`:
 *   1. Full name: "Ramirez, Maria Elena"
 *   2. Abbreviate trailing given names: "Ramirez, Maria E."
 *   3. Abbreviate all given names: "Ramirez, M. E."
 *   4. First initial only: "Ramirez, M."
 *
 * Names without a comma (e.g. "Staff") are returned as-is.
 */
export function abbreviateInstructor(name: string, maxLen = 18): string {
  if (name.length <= maxLen) return name;

  const commaIdx = name.indexOf(", ");
  if (commaIdx === -1) return name;

  const last = name.slice(0, commaIdx);
  const parts = name.slice(commaIdx + 2).split(" ");

  // Level 2: abbreviate trailing given names, keep first given name intact
  // "Maria Elena" → "Maria E."
  if (parts.length > 1) {
    const abbreviated = [parts[0], ...parts.slice(1).map((p) => `${p[0]}.`)].join(" ");
    const result = `${last}, ${abbreviated}`;
    if (result.length <= maxLen) return result;
  }

  // Level 3: abbreviate all given names
  // "Maria Elena" → "M. E."
  if (parts.length > 1) {
    const allInitials = parts.map((p) => `${p[0]}.`).join(" ");
    const result = `${last}, ${allInitials}`;
    if (result.length <= maxLen) return result;
  }

  // Level 4: first initial only
  // "Maria Elena" → "M."  or  "John" → "J."
  return `${last}, ${parts[0][0]}.`;
}

/**
 * Get the primary instructor from a course.
 *
 * When `primaryInstructorId` is available (from the backend), does a direct
 * lookup. Falls back to iterating `isPrimary` / first instructor for safety.
 */
export function getPrimaryInstructor(
  instructors: InstructorResponse[],
  primaryInstructorId?: number | null
): InstructorResponse | undefined {
  if (primaryInstructorId != null) {
    return instructors.find((i) => i.instructorId === primaryInstructorId) ?? instructors[0];
  }
  return instructors.find((i) => i.isPrimary) ?? instructors[0];
}

/** Longer location string using building description: "Main Hall 2.206" */
function formatLocationLong(mt: DbMeetingTime): string | null {
  const name = mt.location?.buildingDescription ?? mt.location?.building;
  if (!name) return null;
  return mt.location?.room ? `${name} ${mt.location.room}` : name;
}

export function formatMeetingDaysVerbose(mt: DbMeetingTime): string {
  return formatDayVerbose(mt.days);
}

/**
 * Full verbose tooltip for a single meeting time:
 * "Tuesdays & Thursdays, 4:15–5:30 PM\nMain Hall 2.206 · Aug 26 – Dec 12, 2024"
 */
export function formatMeetingTimeTooltip(mt: DbMeetingTime): string {
  const days = formatMeetingDaysVerbose(mt);
  const range = formatTimeRange(mt.timeRange?.start ?? null, mt.timeRange?.end ?? null);
  let line1: string;
  if (!days && range === "TBA") {
    line1 = "TBA";
  } else if (!days) {
    line1 = range;
  } else if (range === "TBA") {
    line1 = `${days}, TBA`;
  } else {
    line1 = `${days}, ${range}`;
  }

  const parts = [line1];

  const loc = formatLocationLong(mt);
  const dateRange = `${formatDateShort(mt.dateRange.start)} – ${formatDateShort(mt.dateRange.end)}`;

  if (loc && dateRange) {
    parts.push(`${loc}, ${dateRange}`);
  } else if (loc) {
    parts.push(loc);
  } else if (dateRange) {
    parts.push(dateRange);
  }

  return parts.join("\n");
}

/** Full verbose tooltip for all meeting times on a course, newline-separated. */
export function formatMeetingTimesTooltip(meetingTimes: DbMeetingTime[]): string {
  if (meetingTimes.length === 0) return "TBA";
  return meetingTimes.map(formatMeetingTimeTooltip).join("\n\n");
}

/** Border accent class based on instructional method and campus. */
export function concernAccentClass(
  method: InstructionalMethod | null,
  campus: Campus | null
): string | null {
  if (method?.type === "Online") return "border-l-2 border-l-blue-500";
  if (method?.type === "Hybrid") return "border-l-2 border-l-purple-500";
  if (campus?.type === "OnlinePrograms") return "border-l-2 border-l-cyan-500";
  if (
    campus?.type === "Downtown" ||
    campus?.type === "Southwest" ||
    campus?.type === "Laredo" ||
    campus?.type === "Unknown"
  )
    return "border-l-2 border-l-amber-500";
  return null;
}

/** Tooltip text for the location column: long-form location + delivery note */
export function formatLocationTooltip(course: CourseResponse): string | null {
  const parts: string[] = [];

  for (const mt of course.meetingTimes) {
    const loc = formatLocationLong(mt);
    if (loc && !parts.includes(loc)) parts.push(loc);
  }

  const locationLine = parts.length > 0 ? parts.join(", ") : null;

  // Build delivery note from instructional method
  let deliveryNote: string | null = null;
  const method = course.instructionalMethod;
  if (method) {
    switch (method.type) {
      case "Online":
        deliveryNote =
          method.variant === "Async"
            ? "Online (Async)"
            : method.variant === "Sync"
              ? "Online (Sync)"
              : "Online";
        break;
      case "Hybrid":
        deliveryNote = "Hybrid";
        break;
      case "Independent":
        deliveryNote = "Independent Study";
        break;
    }
  }

  // Add campus restriction note
  if (course.campus?.type === "OnlinePrograms") {
    deliveryNote = deliveryNote ? `${deliveryNote} — Online Programs only` : "Online Programs only";
  }

  if (locationLine && deliveryNote) return `${locationLine}\n${deliveryNote}`;
  if (locationLine) return locationLine;
  if (deliveryNote) return deliveryNote;
  return null;
}

/** Text color class for seat availability: purple (overenrolled), red (full), yellow (low), green (open) */
export function seatsColor(openSeats: number): string {
  if (openSeats < 0) return "text-purple-500";
  if (openSeats === 0) return "text-status-red";
  if (openSeats <= 5) return "text-yellow-500";
  return "text-status-green";
}

/** Background dot color class for seat availability */
export function seatsDotColor(openSeats: number): string {
  if (openSeats < 0) return "bg-purple-500";
  if (openSeats === 0) return "bg-red-500";
  if (openSeats <= 5) return "bg-yellow-500";
  return "bg-green-500";
}

/** RMP professor page URL from legacy ID */
export function rmpUrl(legacyId: number): string {
  return `https://www.ratemyprofessors.com/professor/${legacyId}`;
}

/**
 * Smooth OKLCH color + text-shadow for a RateMyProfessors rating.
 *
 * Three-stop gradient interpolated in OKLCH:
 *   1.0 → red, 3.0 → amber, 5.0 → green
 * with separate light/dark mode tuning.
 */
export function ratingStyle(rating: number, isDark: boolean): string {
  const clamped = Math.max(1, Math.min(5, rating));

  // OKLCH stops: [lightness, chroma, hue]
  const stops: { light: [number, number, number]; dark: [number, number, number] }[] = [
    { light: [0.63, 0.2, 25], dark: [0.7, 0.19, 25] }, // 1.0 – red
    { light: [0.7, 0.16, 85], dark: [0.78, 0.15, 85] }, // 3.0 – amber
    { light: [0.65, 0.2, 145], dark: [0.72, 0.19, 145] }, // 5.0 – green
  ];

  let t: number;
  let fromIdx: number;
  if (clamped <= 3) {
    t = (clamped - 1) / 2;
    fromIdx = 0;
  } else {
    t = (clamped - 3) / 2;
    fromIdx = 1;
  }

  const from = isDark ? stops[fromIdx].dark : stops[fromIdx].light;
  const to = isDark ? stops[fromIdx + 1].dark : stops[fromIdx + 1].light;

  const l = from[0] + (to[0] - from[0]) * t;
  const c = from[1] + (to[1] - from[1]) * t;
  const h = from[2] + (to[2] - from[2]) * t;

  return `color: oklch(${l.toFixed(3)} ${c.toFixed(3)} ${h.toFixed(1)}); text-shadow: 0 0 4px oklch(${l.toFixed(3)} ${c.toFixed(3)} ${h.toFixed(1)} / 0.3);`;
}

/**
 * Returns the interpolated OKLCH color string for a rating value.
 * Use this when you need the raw color for multiple CSS properties.
 */
export function ratingColor(rating: number, isDark: boolean): string {
  const clamped = Math.max(1, Math.min(5, rating));

  const stops: { light: [number, number, number]; dark: [number, number, number] }[] = [
    { light: [0.63, 0.2, 25], dark: [0.7, 0.19, 25] },
    { light: [0.7, 0.16, 85], dark: [0.78, 0.15, 85] },
    { light: [0.65, 0.2, 145], dark: [0.72, 0.19, 145] },
  ];

  let t: number;
  let fromIdx: number;
  if (clamped <= 3) {
    t = (clamped - 1) / 2;
    fromIdx = 0;
  } else {
    t = (clamped - 3) / 2;
    fromIdx = 1;
  }

  const from = isDark ? stops[fromIdx].dark : stops[fromIdx].light;
  const to = isDark ? stops[fromIdx + 1].dark : stops[fromIdx + 1].light;

  const l = from[0] + (to[0] - from[0]) * t;
  const c = from[1] + (to[1] - from[1]) * t;
  const h = from[2] + (to[2] - from[2]) * t;

  return `oklch(${l.toFixed(3)} ${c.toFixed(3)} ${h.toFixed(1)})`;
}

/**
 * Returns inline style string for a score badge: text color, background tint, and text shadow.
 */
export function scoreBadgeStyle(rating: number, isDark: boolean): string {
  const color = ratingColor(rating, isDark);
  return `color: ${color}; background-color: ${color.replace(")", " / 0.1)")}; text-shadow: 0 0 4px ${color.replace(")", " / 0.3)")};`;
}

/** Format credit hours display */
export function formatCreditHours(course: CourseResponse): string {
  if (course.creditHours == null) return "—";
  if (course.creditHours.type === "fixed") {
    return String(course.creditHours.hours);
  }
  return `${course.creditHours.low}–${course.creditHours.high}`;
}

/**
 * Format an instructor's name for display.
 *
 * When an `InstructorResponse` object with `firstName` and `lastName` is
 * provided, uses them directly: "First Last". Otherwise falls back to parsing
 * `displayName` from Banner's "Last, First Middle" format.
 */
export function formatInstructorName(
  nameOrInstructor: string | Pick<InstructorResponse, "displayName" | "firstName" | "lastName">
): string {
  if (typeof nameOrInstructor !== "string") {
    const { firstName, lastName, displayName } = nameOrInstructor;
    if (firstName && lastName) return `${firstName} ${lastName}`;
    return formatInstructorName(displayName);
  }

  const displayName = nameOrInstructor;
  const commaIdx = displayName.indexOf(",");
  if (commaIdx === -1) return displayName.trim();

  const last = displayName.slice(0, commaIdx).trim();
  const rest = displayName.slice(commaIdx + 1).trim();
  if (!rest) return last;

  return `${rest} ${last}`;
}

/** Compact meeting time summary for mobile cards: "MWF 9:00–9:50 AM", "Async", or "TBA" */
export function formatMeetingTimeSummary(course: CourseResponse): string {
  if (course.isAsyncOnline) return "Async";
  if (course.meetingTimes.length === 0) return "TBA";
  const mt = course.meetingTimes[0];
  if (mt.days.length === 0 && mt.timeRange === null) return "TBA";
  return `${formatMeetingDays(mt)} ${formatTimeRange(mt.timeRange?.start ?? null, mt.timeRange?.end ?? null)}`;
}
