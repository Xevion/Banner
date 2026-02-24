/**
 * Pure canvas rendering functions for the timeline chart.
 *
 * Every function takes a {@link ChartContext} plus any data it needs.
 * No Svelte reactivity, no side-effects beyond drawing on the context.
 */
import { type Series, area, curveMonotoneX, stack } from "d3-shape";
import { timeFormat } from "d3-time-format";

import type { AnimMap } from "./animation";
import {
  AREA_FILL_ALPHA,
  AREA_STROKE_ALPHA,
  AXIS_FONT,
  GRID_ALPHA,
  HOUR_GRID_ALPHA,
  HOVER_HIGHLIGHT_ALPHA,
  NOW_LABEL_FONT,
  NOW_LINE_COLOR,
  NOW_LINE_WIDTH,
  NOW_TRIANGLE_HALF_WIDTH,
  NOW_TRIANGLE_HEIGHT,
  SETTLE_THRESHOLD,
  SLOT_INTERVAL_MS,
} from "./constants";
import { getSubjectColor } from "./data";
import type { ChartContext, TimeSlot } from "./types";
import { getStackSubjects } from "./viewport";

const fmtHour = timeFormat("%-I %p");
const fmtAxisDetailed = timeFormat("%-I:%M %p");
const fmtAxisCoarse = timeFormat("%-I %p");
const fmtNow = timeFormat("%-I:%M %p");

type StackPoint = Series<TimeSlot, string>[number];
export type VisibleStack = Series<TimeSlot, string>[];

/** Choose the number of x-axis ticks based on the viewport span. */
export function chooseTickCount(viewSpan: number): number {
  const spanHours = viewSpan / (60 * 60 * 1000);
  if (spanHours <= 1) return 12;
  if (spanHours <= 3) return 12;
  if (spanHours <= 8) return 16;
  if (spanHours <= 14) return 14;
  return 10;
}

/**
 * Stack only the visible slice using *animated* values so transitions
 * between filter/data states are smooth. Includes subjects that are
 * still animating out so removal is gradual.
 *
 * @param allSubjects - full set of known subject codes
 */
export function stackVisibleSlots(
  visible: TimeSlot[],
  allSubjects: readonly string[],
  enabledSubjects: Set<string>,
  animMap: AnimMap
): VisibleStack {
  if (visible.length === 0) return [];

  const stackKeys = getStackSubjects(
    visible,
    allSubjects,
    enabledSubjects,
    animMap,
    SETTLE_THRESHOLD
  );
  if (stackKeys.length === 0) return [];

  // Build synthetic slots with animated current values.
  const animatedSlots: TimeSlot[] = visible.map((slot) => {
    const timeMs = slot.time.getTime();
    const subjectMap = animMap.get(timeMs);
    const subjects: Record<string, number> = {};
    for (const subject of stackKeys) {
      const entry = subjectMap?.get(subject);
      subjects[subject] = entry ? entry.current : slot.subjects[subject] || 0;
    }
    return { time: slot.time, subjects };
  });

  const gen = stack<TimeSlot>()
    .keys(stackKeys)
    .value((d, key) => d.subjects[key] || 0);
  return gen(animatedSlots);
}

export function drawGrid(chart: ChartContext): void {
  const { ctx, xScale, chartTop, chartBottom, width, viewSpan, viewStart, viewEnd } = chart;
  const tickCount = chooseTickCount(viewSpan);

  ctx.save();
  ctx.lineWidth = 1;

  // Minor tick grid lines
  const ticks = xScale.ticks(tickCount);
  ctx.strokeStyle = `rgba(128, 128, 128, ${GRID_ALPHA})`;
  for (const tick of ticks) {
    if (tick.getMinutes() === 0) continue;
    const x = Math.round(xScale(tick)) + 0.5;
    if (x < 0 || x > width) continue;
    ctx.beginPath();
    ctx.moveTo(x, chartTop);
    ctx.lineTo(x, chartBottom);
    ctx.stroke();
  }

  // Hourly grid lines with labels
  const spanHours = viewSpan / (60 * 60 * 1000);
  let hourStep = 1;
  if (spanHours > 48) hourStep = 6;
  else if (spanHours > 24) hourStep = 4;
  else if (spanHours > 12) hourStep = 2;

  const startHour = new Date(viewStart);
  startHour.setMinutes(0, 0, 0);
  if (hourStep > 1) {
    const h = startHour.getHours();
    startHour.setHours(Math.ceil(h / hourStep) * hourStep);
  } else if (startHour.getTime() < viewStart) {
    startHour.setHours(startHour.getHours() + 1);
  }

  const hourStepMs = hourStep * 60 * 60 * 1000;
  for (let t = startHour.getTime(); t <= viewEnd; t += hourStepMs) {
    const d = new Date(t);
    const x = Math.round(xScale(d)) + 0.5;
    if (x < 0 || x > width) continue;

    ctx.strokeStyle = `rgba(128, 128, 128, ${HOUR_GRID_ALPHA})`;
    ctx.beginPath();
    ctx.moveTo(x, chartTop);
    ctx.lineTo(x, chartBottom);
    ctx.stroke();

    ctx.fillText(fmtHour(d), x + 5, chartTop + 6);
  }

  ctx.restore();
}

/**
 * Trace the top outline of the stacked area onto `ctx` as a clip path.
 */
function traceStackOutline(chart: ChartContext, visibleStack: VisibleStack): void {
  if (visibleStack.length === 0) return;
  const { ctx, xScale, yScale } = chart;
  const topLayer = visibleStack[visibleStack.length - 1];

  ctx.beginPath();
  area<StackPoint>()
    .x((d) => xScale(d.data.time))
    .y0(() => yScale(0))
    .y1((d) => yScale(d[1]))
    .curve(curveMonotoneX)
    .context(ctx)(topLayer as unknown as StackPoint[]);
}

export function drawHoverColumn(
  chart: ChartContext,
  visibleStack: VisibleStack,
  hoverSlotTime: number | null
): void {
  if (hoverSlotTime == null || visibleStack.length === 0) return;
  const { ctx, xScale, chartTop, chartBottom } = chart;

  const x0 = xScale(new Date(hoverSlotTime));
  const x1 = xScale(new Date(hoverSlotTime + SLOT_INTERVAL_MS));

  ctx.save();
  traceStackOutline(chart, visibleStack);
  ctx.clip();

  ctx.fillStyle = `rgba(255, 255, 255, ${HOVER_HIGHLIGHT_ALPHA})`;
  ctx.fillRect(x0, chartTop, x1 - x0, chartBottom - chartTop);
  ctx.restore();
}

export function drawStackedArea(chart: ChartContext, visibleStack: VisibleStack): void {
  const { ctx, xScale, yScale, width, chartTop, chartBottom } = chart;

  ctx.save();
  ctx.beginPath();
  ctx.rect(0, chartTop, width, chartBottom - chartTop);
  ctx.clip();

  for (let i = visibleStack.length - 1; i >= 0; i--) {
    const layer = visibleStack[i];
    const subject = layer.key;
    const color = getSubjectColor(subject);

    ctx.beginPath();
    area<StackPoint>()
      .x((d) => xScale(d.data.time))
      .y0((d) => yScale(d[0]))
      .y1((d) => yScale(d[1]))
      .curve(curveMonotoneX)
      .context(ctx)(layer as unknown as StackPoint[]);

    ctx.globalAlpha = AREA_FILL_ALPHA;
    ctx.fillStyle = color;
    ctx.fill();

    ctx.globalAlpha = AREA_STROKE_ALPHA;
    ctx.strokeStyle = color;
    ctx.lineWidth = 1;
    ctx.stroke();
  }

  ctx.restore();
}

export function drawNowLine(chart: ChartContext): void {
  const { ctx, xScale, chartTop, chartBottom } = chart;
  const now = new Date();
  const x = xScale(now);
  if (x < 0 || x > chart.width) return;

  ctx.save();

  ctx.shadowColor = "rgba(239, 68, 68, 0.5)";
  ctx.shadowBlur = 8;
  ctx.strokeStyle = NOW_LINE_COLOR;
  ctx.lineWidth = NOW_LINE_WIDTH;
  ctx.setLineDash([]);

  ctx.beginPath();
  ctx.moveTo(x, chartTop);
  ctx.lineTo(x, chartBottom);
  ctx.stroke();

  ctx.shadowBlur = 0;
  ctx.fillStyle = NOW_LINE_COLOR;

  // Triangle marker at chart top
  ctx.beginPath();
  ctx.moveTo(x - NOW_TRIANGLE_HALF_WIDTH, chartTop);
  ctx.lineTo(x + NOW_TRIANGLE_HALF_WIDTH, chartTop);
  ctx.lineTo(x, chartTop + NOW_TRIANGLE_HEIGHT);
  ctx.closePath();
  ctx.fill();

  // Time label
  ctx.font = NOW_LABEL_FONT;
  ctx.textAlign = "left";
  ctx.textBaseline = "bottom";
  ctx.fillText(fmtNow(now), x + 6, chartTop - 1);

  ctx.restore();
}

export function drawTimeAxis(chart: ChartContext): void {
  const { ctx, xScale, width, chartBottom, viewSpan } = chart;
  const tickCount = chooseTickCount(viewSpan);
  const y = chartBottom;
  const ticks = xScale.ticks(tickCount);

  ctx.save();

  // Axis baseline
  ctx.strokeStyle = "rgba(128, 128, 128, 0.15)";
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.moveTo(0, y + 0.5);
  ctx.lineTo(width, y + 0.5);
  ctx.stroke();

  const spanHours = viewSpan / (60 * 60 * 1000);
  const fmt = spanHours <= 3 ? fmtAxisDetailed : fmtAxisCoarse;

  ctx.fillStyle = "rgba(128, 128, 128, 0.6)";
  ctx.font = AXIS_FONT;
  ctx.textAlign = "center";
  ctx.textBaseline = "top";

  for (const tick of ticks) {
    const x = xScale(tick);
    if (x < 20 || x > width - 20) continue;

    ctx.strokeStyle = "rgba(128, 128, 128, 0.2)";
    ctx.beginPath();
    ctx.moveTo(x, y);
    ctx.lineTo(x, y + 4);
    ctx.stroke();

    ctx.fillText(fmt(tick), x, y + 6);
  }

  ctx.restore();
}
