/**
 * Animation-map management for the timeline's stacked-area transitions.
 *
 * Each visible slot has per-subject animated values that lerp toward
 * targets. This module owns the AnimMap lifecycle: syncing targets,
 * stepping current values, and pruning offscreen entries.
 */
import { MAXY_EASE, MIN_MAXY, SETTLE_THRESHOLD, VALUE_EASE } from "./constants";
import type { AnimEntry, TimeSlot } from "./types";

export type AnimMap = Map<number, Map<string, AnimEntry>>;

/** Create a fresh, empty animation map. */
export function createAnimMap(): AnimMap {
  return new Map();
}

/**
 * Sync animMap targets from data + filter state.
 * New slots start at current=0 so they animate in from the baseline.
 * Disabled subjects get target=0 so they animate out.
 *
 * @param subjects - the full list of known subject codes
 * @param enabledSubjects - subjects currently toggled on
 */
export function syncAnimTargets(
  animMap: AnimMap,
  slots: TimeSlot[],
  subjects: readonly string[],
  enabledSubjects: Set<string>
): void {
  for (const slot of slots) {
    const timeMs = slot.time.getTime();
    let subjectMap = animMap.get(timeMs);
    if (!subjectMap) {
      subjectMap = new Map();
      animMap.set(timeMs, subjectMap);
    }

    for (const subject of subjects) {
      const realValue = enabledSubjects.has(subject) ? (slot.subjects[subject] ?? 0) : 0;
      const entry = subjectMap.get(subject);
      if (entry) {
        entry.target = realValue;
      } else {
        subjectMap.set(subject, { current: 0, target: realValue });
      }
    }
  }
}

/**
 * Advance all animated values toward their targets.
 * Returns the new animatedMaxY value and whether any entry is still moving.
 */
export function stepAnimations(
  animMap: AnimMap,
  dt: number,
  prevMaxY: number
): { animating: boolean; maxY: number } {
  const ease = 1 - Math.pow(1 - VALUE_EASE, dt / 16);
  let animating = false;
  let computedMaxY = MIN_MAXY;

  for (const subjectMap of animMap.values()) {
    let slotSum = 0;
    for (const entry of subjectMap.values()) {
      const diff = entry.target - entry.current;
      if (Math.abs(diff) < SETTLE_THRESHOLD) {
        if (entry.current !== entry.target) {
          entry.current = entry.target;
        }
      } else {
        entry.current += diff * ease;
        animating = true;
      }
      slotSum += entry.current;
    }
    if (slotSum > computedMaxY) computedMaxY = slotSum;
  }

  const maxYEase = 1 - Math.pow(1 - MAXY_EASE, dt / 16);
  const maxDiff = computedMaxY - prevMaxY;
  let maxY: number;
  if (Math.abs(maxDiff) < SETTLE_THRESHOLD) {
    maxY = computedMaxY;
  } else {
    maxY = prevMaxY + maxDiff * maxYEase;
    animating = true;
  }

  return { animating, maxY };
}

/**
 * Remove animMap entries whose timestamps fall outside the viewport
 * (with margin), preventing unbounded memory growth during long sessions.
 */
export function pruneAnimMap(
  animMap: AnimMap,
  viewStart: number,
  viewEnd: number,
  viewSpan: number
): void {
  const margin = viewSpan;
  for (const timeMs of animMap.keys()) {
    if (timeMs < viewStart - margin || timeMs > viewEnd + margin) {
      animMap.delete(timeMs);
    }
  }
}
