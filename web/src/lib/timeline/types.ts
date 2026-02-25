/**
 * Shared types for the timeline feature.
 *
 * Subjects are dynamic strings (actual Banner subject codes like "CS",
 * "MAT", "BIO") rather than a fixed enum -- the set of subjects comes
 * from the API response.
 */
import type { ScaleLinear, ScaleTime } from "d3-scale";

/** A single 15-minute time slot with per-subject enrollment totals. */
export interface TimeSlot {
  time: Date;
  subjects: Record<string, number>;
}

/** Lerped animation entry for a single subject within a slot. */
export interface AnimEntry {
  current: number;
  target: number;
}

/**
 * Shared context passed to all canvas rendering functions.
 * Computed once per frame in the orchestrator and threaded through.
 */
export interface ChartContext {
  ctx: CanvasRenderingContext2D;
  xScale: ScaleTime<number, number>;
  yScale: ScaleLinear<number, number>;
  width: number;
  chartTop: number;
  chartBottom: number;
  viewSpan: number;
  viewStart: number;
  viewEnd: number;
}
