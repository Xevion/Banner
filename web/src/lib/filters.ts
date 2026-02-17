import type { CodeDescription } from "$lib/bindings";
export { DAY_OPTIONS } from "$lib/days";

export function toggleDay(days: string[], day: string): string[] {
  return days.includes(day) ? days.filter((d) => d !== day) : [...days, day];
}

export function parseTimeInput(input: string): string | null {
  const trimmed = input.trim();
  if (trimmed === "") return null;

  const ampmMatch = /^(\d{1,2}):(\d{2})\s*(AM|PM)$/i.exec(trimmed);
  if (ampmMatch) {
    let hours = parseInt(ampmMatch[1], 10);
    const minutes = parseInt(ampmMatch[2], 10);
    const period = ampmMatch[3].toUpperCase();
    if (period === "PM" && hours !== 12) hours += 12;
    if (period === "AM" && hours === 12) hours = 0;
    return String(hours).padStart(2, "0") + String(minutes).padStart(2, "0");
  }

  const militaryMatch = /^(\d{1,2}):(\d{2})$/.exec(trimmed);
  if (militaryMatch) {
    const hours = parseInt(militaryMatch[1], 10);
    const minutes = parseInt(militaryMatch[2], 10);
    return String(hours).padStart(2, "0") + String(minutes).padStart(2, "0");
  }

  return null;
}

export function formatCompactTime(time: string | null): string {
  if (time?.length !== 4) return "";
  const hours = parseInt(time.slice(0, 2), 10);
  const minutes = time.slice(2);
  const period = hours >= 12 ? "PM" : "AM";
  const displayHours = hours === 0 ? 12 : hours > 12 ? hours - 12 : hours;
  return `${displayHours}:${minutes} ${period}`;
}

export function toggleValue(arr: string[], code: string): string[] {
  return arr.includes(code) ? arr.filter((v) => v !== code) : [...arr, code];
}

export interface GroupedAttributes {
  core: CodeDescription[];
  level: CodeDescription[];
  special: CodeDescription[];
}

export function groupAttributes(
  attributes: CodeDescription[],
  groups: { core: Set<string>; level: Set<string> }
): GroupedAttributes {
  const core: CodeDescription[] = [];
  const level: CodeDescription[] = [];
  const special: CodeDescription[] = [];

  for (const attr of attributes) {
    if (groups.core.has(attr.filterValue)) {
      core.push(attr);
    } else if (groups.level.has(attr.filterValue)) {
      level.push(attr);
    } else {
      special.push(attr);
    }
  }

  return { core, level, special };
}
