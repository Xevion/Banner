import type { MetricEntry } from "$lib/bindings";

/** One enrollment snapshot, with capacity derived and the timestamp parsed. */
export interface HistoryPoint {
  date: Date;
  enrolled: number;
  waiting: number;
  open: number;
  capacity: number;
}

/**
 * Below this many snapshots a chart has no shape to show, and the timeline reads better.
 * Most sections never reach it -- the scraper only writes a row when something changes.
 */
export const CHART_MIN_POINTS = 4;

/** Oldest-first, with capacity derived. The API returns newest-first. */
export function toHistoryPoints(metrics: MetricEntry[]): HistoryPoint[] {
  return metrics
    .map((m) => ({
      date: new Date(m.timestamp),
      enrolled: m.enrollment,
      waiting: m.waitCount,
      open: m.seatsAvailable,
      capacity: m.enrollment + m.seatsAvailable,
    }))
    .sort((a, b) => a.date.getTime() - b.date.getTime());
}

/** Largest value any mark has to reach, so capacity changes stay visible on a shared scale. */
export function historyScaleMax(points: HistoryPoint[]): number {
  return points.reduce(
    (max, p) => Math.max(max, p.capacity, p.enrolled, p.capacity + p.waiting),
    1
  );
}

/** True once the waitlist leaves zero anywhere in the series. */
export function hasWaitlist(points: HistoryPoint[]): boolean {
  return points.some((p) => p.waiting > 0);
}

/**
 * One row per day, carrying the last known value forward. Days the scraper saw no change
 * are marked `held` so a flat stretch reads as nothing happening rather than as missing data.
 */
export interface DayRow extends HistoryPoint {
  held: boolean;
}

export function toDayRows(points: HistoryPoint[]): DayRow[] {
  if (points.length === 0) return [];

  const DAY_MS = 86_400_000;
  const dayOf = (d: Date) => Math.floor(d.getTime() / DAY_MS);

  const lastOfDay = new Map<number, HistoryPoint>();
  for (const p of points) lastOfDay.set(dayOf(p.date), p);

  const firstDay = dayOf(points[0].date);
  const lastDay = dayOf(points[points.length - 1].date);

  const rows: DayRow[] = [];
  let carry = points[0];
  for (let day = firstDay; day <= lastDay; day++) {
    const seen = lastOfDay.get(day);
    if (seen) carry = seen;
    rows.push({ ...carry, date: new Date(day * DAY_MS), held: !seen });
  }
  return rows.reverse();
}
