/**
 * Pure viewport utility functions: binary search, visible-slot slicing,
 * hit-testing, and snapping for the timeline canvas.
 */
import { RENDER_MARGIN_SLOTS, SLOT_INTERVAL_MS } from "./constants";
import type { TimeSlot } from "./types";

/**
 * Binary-search for the index of the first slot whose time >= target.
 * `slots` must be sorted ascending by time.
 */
export function lowerBound(slots: TimeSlot[], targetMs: number): number {
  let lo = 0;
  let hi = slots.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    const slot = slots[mid];
    if (slot !== undefined && slot.time.getTime() < targetMs) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

/**
 * Return the sub-array of `data` covering the viewport [viewStart, viewEnd]
 * plus a small margin for smooth curve edges.
 */
export function getVisibleSlots(data: TimeSlot[], viewStart: number, viewEnd: number): TimeSlot[] {
  if (data.length === 0) return data;
  const lo = Math.max(0, lowerBound(data, viewStart) - RENDER_MARGIN_SLOTS);
  const hi = Math.min(data.length, lowerBound(data, viewEnd) + RENDER_MARGIN_SLOTS);
  return data.slice(lo, hi);
}

/** Find the slot closest to `timeMs`, or null if none within one interval. */
export function findSlotByTime(data: TimeSlot[], timeMs: number): TimeSlot | null {
  if (data.length === 0) return null;
  const idx = lowerBound(data, timeMs);
  let best: TimeSlot | null = null;
  let bestDist = Infinity;
  for (const i of [idx - 1, idx]) {
    const slot = data[i];
    if (!slot) continue;
    const dist = Math.abs(slot.time.getTime() - timeMs);
    if (dist < bestDist) {
      bestDist = dist;
      best = slot;
    }
  }
  if (best && bestDist < SLOT_INTERVAL_MS) return best;
  return null;
}

/** Snap a timestamp down to the nearest 15-minute slot boundary. */
export function snapToSlot(timeMs: number): number {
  return Math.floor(timeMs / SLOT_INTERVAL_MS) * SLOT_INTERVAL_MS;
}

/** Sum of enrollment counts for enabled subjects in a slot. */
export function enabledTotalClasses(slot: TimeSlot, activeSubjects: readonly string[]): number {
  let sum = 0;
  for (const s of activeSubjects) {
    sum += slot.subjects[s] ?? 0;
  }
  return sum;
}

/**
 * Determine which subjects to include in the stack: all enabled subjects
 * plus any disabled subjects still animating out (current > threshold).
 *
 * @param allSubjects - the full set of known subject codes
 */
export function getStackSubjects(
  visible: TimeSlot[],
  allSubjects: readonly string[],
  enabledSubjects: Set<string>,
  animMap: Map<number, Map<string, { current: number }>>,
  settleThreshold: number
): string[] {
  const subjects: string[] = [];
  for (const subject of allSubjects) {
    if (enabledSubjects.has(subject)) {
      subjects.push(subject);
      continue;
    }
    for (const slot of visible) {
      const entry = animMap.get(slot.time.getTime())?.get(subject);
      if (entry && Math.abs(entry.current) > settleThreshold) {
        subjects.push(subject);
        break;
      }
    }
  }
  return subjects;
}
