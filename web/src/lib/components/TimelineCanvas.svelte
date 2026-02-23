<script lang="ts">
import { scaleLinear, scaleTime } from "d3-scale";
import { onMount } from "svelte";
import { SvelteMap, SvelteSet } from "svelte/reactivity";

import {
  createAnimMap,
  pruneAnimMap,
  stepAnimations,
  syncAnimTargets,
} from "$lib/timeline/animation";
import {
  CHART_HEIGHT_RATIO,
  DEFAULT_AXIS_RATIO,
  DEFAULT_DT,
  DEFAULT_SPAN_MS,
  FOLLOW_EASE,
  MAX_DT,
  MAX_SPAN_MS,
  MIN_MAXY,
  MIN_SPAN_MS,
  PADDING,
  PAN_EASE,
  PAN_FRICTION,
  PAN_SETTLE_THRESHOLD_PX,
  PAN_STEP_CTRL_RATIO,
  PAN_STEP_RATIO,
  PAN_STOP_THRESHOLD,
  PAN_STOP_THRESHOLD_Y,
  TAP_MAX_DISTANCE_PX,
  TAP_MAX_DURATION_MS,
  VELOCITY_MIN_DT,
  VELOCITY_SAMPLE_WINDOW,
  YRATIO_MAX,
  YRATIO_MIN,
  YRATIO_SETTLE_THRESHOLD,
  YRATIO_STEP,
  ZOOM_EASE,
  ZOOM_FACTOR,
  ZOOM_KEY_FACTOR,
  ZOOM_SETTLE_THRESHOLD,
} from "$lib/timeline/constants";
import {
  drawGrid,
  drawHoverColumn,
  drawNowLine,
  drawStackedArea,
  drawTimeAxis,
  stackVisibleSlots,
} from "$lib/timeline/renderer";
import { createTimelineStore } from "$lib/timeline/store.svelte";
import type { ChartContext, TimeSlot } from "$lib/timeline/types";
import {
  enabledTotalClasses,
  findSlotByTime,
  getVisibleSlots,
  snapToSlot,
} from "$lib/timeline/viewport";
import TimelineDrawer from "./TimelineDrawer.svelte";
import TimelineTooltip from "./TimelineTooltip.svelte";

// ── Reactive DOM state ──────────────────────────────────────────────
let canvasEl: HTMLCanvasElement | undefined = $state();
let containerEl: HTMLDivElement | undefined = $state();
let width = $state(800);
let height = $state(400);
let dpr = $state(1);

// ── View window ─────────────────────────────────────────────────────
let viewCenter = $state(Date.now());
let viewSpan = $state(DEFAULT_SPAN_MS);
let viewYRatio = $state(DEFAULT_AXIS_RATIO);

// ── Interaction state ───────────────────────────────────────────────
let isDragging = $state(false);
let dragStartX = $state(0);
let dragStartY = $state(0);
let dragStartCenter = $state(0);
let dragStartYRatio = $state(0);
let followEnabled = $state(true);
let ctrlHeld = $state(false);

// ── Animation state (intentionally non-reactive — updated in rAF) ──
let panVelocity = 0;
let panVelocityY = 0;
let pointerSamples: { time: number; x: number; y: number }[] = [];

// ── Multi-touch / pinch state ────────────────────────────────────────
let activePointers = new SvelteMap<number, { x: number; y: number }>();
let isPinching = false;
let pinchStartDist = 0;
let pinchStartSpan = 0;
let pinchAnchorTime = 0;
let pinchAnchorRatio = 0.5;

// ── Tap detection ────────────────────────────────────────────────────
let pointerDownTime = 0;
let pointerDownPos = { x: 0, y: 0 };

let targetSpan = DEFAULT_SPAN_MS;
let zoomAnchorTime = 0;
let zoomAnchorRatio = 0.5;
let isZoomAnimating = false;

let targetCenter = Date.now();
let isPanAnimating = false;
let targetYRatio = DEFAULT_AXIS_RATIO;
let isYPanAnimating = false;

let animationFrameId = 0;
let lastFrameTime = 0;
let animatedMaxY = MIN_MAXY;

const animMap = createAnimMap();

// ── Tooltip + hover ─────────────────────────────────────────────────
let tooltipVisible = $state(false);
let tooltipX = $state(0);
let tooltipY = $state(0);
let tooltipSlot: TimeSlot | null = $state(null);
let hoverSlotTime: number | null = $state(null);
let lastPointerClientX = 0;
let lastPointerClientY = 0;
let pointerOverCanvas = false;

let drawerOpen = $state(false);
let enabledSubjects = new SvelteSet<string>();

const store = createTimelineStore();
let data: TimeSlot[] = $derived(store.data);
let allSubjects: string[] = $derived(store.subjects);

$effect(() => {
  const storeSubjects = store.subjects;
  for (const s of storeSubjects) {
    if (!enabledSubjects.has(s)) {
      enabledSubjects.add(s);
    }
  }
});

let activeSubjects = $derived(allSubjects.filter((s) => enabledSubjects.has(s)));

let viewStart = $derived(viewCenter - viewSpan / 2);
let viewEnd = $derived(viewCenter + viewSpan / 2);
let chartHeight = $derived(height * CHART_HEIGHT_RATIO);
let chartBottom = $derived(height * viewYRatio);
let chartTop = $derived(chartBottom - chartHeight);

let xScale = $derived(
  scaleTime()
    .domain([new Date(viewStart), new Date(viewEnd)])
    .range([PADDING.left, width - PADDING.right])
);

let yScale = scaleLinear()
  .domain([0, MIN_MAXY * 1.1])
  .range([0, 1]);

function toggleSubject(subject: string) {
  if (enabledSubjects.has(subject)) enabledSubjects.delete(subject);
  else enabledSubjects.add(subject);
}

function enableAll() {
  enabledSubjects.clear();
  for (const s of allSubjects) enabledSubjects.add(s);
}

function disableAll() {
  enabledSubjects.clear();
}

function render() {
  if (!canvasEl) return;
  const ctx = canvasEl.getContext("2d");
  if (!ctx) return;

  yScale.domain([0, animatedMaxY * 1.1]).range([chartBottom, chartTop]);

  ctx.save();
  ctx.scale(dpr, dpr);
  ctx.clearRect(0, 0, width, height);

  const chart: ChartContext = {
    ctx,
    xScale,
    yScale,
    width,
    chartTop,
    chartBottom,
    viewSpan,
    viewStart,
    viewEnd,
  };

  const visible = getVisibleSlots(data, viewStart, viewEnd);
  const visibleStack = stackVisibleSlots(visible, allSubjects, enabledSubjects, animMap);

  drawGrid(chart);
  drawHoverColumn(chart, visibleStack, hoverSlotTime);
  drawStackedArea(chart, visibleStack);
  drawNowLine(chart);
  drawTimeAxis(chart);

  ctx.restore();
}

function updateHover() {
  if (!pointerOverCanvas || isDragging || !canvasEl) return;

  const rect = canvasEl.getBoundingClientRect();
  const x = lastPointerClientX - rect.left;
  const y = lastPointerClientY - rect.top;

  if (y < chartTop || y > chartBottom) {
    tooltipVisible = false;
    hoverSlotTime = null;
    return;
  }

  const time = xScale.invert(x);
  const snappedTime = snapToSlot(time.getTime());
  const slot = findSlotByTime(data, snappedTime);
  if (!slot) {
    tooltipVisible = false;
    hoverSlotTime = null;
    return;
  }

  const total = enabledTotalClasses(slot, activeSubjects);
  if (total <= 0) {
    tooltipVisible = false;
    hoverSlotTime = null;
    return;
  }

  if (!ctrlHeld) {
    const stackTopY = yScale(total);
    if (y < stackTopY) {
      tooltipVisible = false;
      hoverSlotTime = null;
      return;
    }
  }

  tooltipSlot = slot;
  tooltipX = lastPointerClientX;
  tooltipY = lastPointerClientY;
  tooltipVisible = true;
  hoverSlotTime = snappedTime;
}

function pinchDistance(): number {
  const pts = [...activePointers.values()];
  if (pts.length < 2) return 0;
  const dx = pts[1].x - pts[0].x;
  const dy = pts[1].y - pts[0].y;
  return Math.hypot(dx, dy);
}

function pinchMidpoint(): { x: number; y: number } {
  const pts = [...activePointers.values()];
  if (pts.length < 2) return { x: 0, y: 0 };
  return { x: (pts[0].x + pts[1].x) / 2, y: (pts[0].y + pts[1].y) / 2 };
}

function onPointerDown(e: PointerEvent) {
  if (e.button !== 0) return;
  (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  activePointers.set(e.pointerId, { x: e.clientX, y: e.clientY });

  if (activePointers.size === 2) {
    isDragging = false;
    isPinching = true;
    pinchStartDist = pinchDistance();
    pinchStartSpan = viewSpan;

    const mid = pinchMidpoint();
    const rect = canvasEl?.getBoundingClientRect();
    const midX = rect ? mid.x - rect.left : mid.x;
    const chartWidth = width - PADDING.left - PADDING.right;

    pinchAnchorTime = xScale.invert(midX).getTime();
    pinchAnchorRatio = (midX - PADDING.left) / chartWidth;
    return;
  }

  isDragging = true;
  dragStartX = e.clientX;
  dragStartY = e.clientY;
  dragStartCenter = viewCenter;
  dragStartYRatio = viewYRatio;
  followEnabled = false;
  panVelocity = 0;
  panVelocityY = 0;
  isZoomAnimating = false;
  isPanAnimating = false;
  isYPanAnimating = false;
  targetSpan = viewSpan;
  tooltipVisible = false;
  hoverSlotTime = null;
  pointerDownTime = performance.now();
  pointerDownPos = { x: e.clientX, y: e.clientY };
  pointerSamples = [{ time: performance.now(), x: e.clientX, y: e.clientY }];
}

function onPointerMove(e: PointerEvent) {
  ctrlHeld = e.ctrlKey || e.metaKey;
  lastPointerClientX = e.clientX;
  lastPointerClientY = e.clientY;
  pointerOverCanvas = true;
  activePointers.set(e.pointerId, { x: e.clientX, y: e.clientY });

  if (isPinching && activePointers.size >= 2) {
    const dist = pinchDistance();
    if (pinchStartDist > 0) {
      const scale = pinchStartDist / dist;
      const newSpan = Math.min(MAX_SPAN_MS, Math.max(MIN_SPAN_MS, pinchStartSpan * scale));
      viewSpan = newSpan;
      targetSpan = newSpan;
      viewCenter = pinchAnchorTime + (0.5 - pinchAnchorRatio) * viewSpan;
    }
    return;
  }

  if (isDragging) {
    const dx = e.clientX - dragStartX;
    const dy = e.clientY - dragStartY;
    const msPerPx = viewSpan / (width - PADDING.left - PADDING.right);
    viewCenter = dragStartCenter - dx * msPerPx;
    viewYRatio = dragStartYRatio + dy / height;

    const now = performance.now();
    pointerSamples.push({ time: now, x: e.clientX, y: e.clientY });
    const cutoff = now - VELOCITY_SAMPLE_WINDOW;
    pointerSamples = pointerSamples.filter((s) => s.time >= cutoff);
  } else {
    updateHover();
  }
}

function onPointerUp(e: PointerEvent) {
  (e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId);
  activePointers.delete(e.pointerId);

  if (isPinching) {
    if (activePointers.size < 2) {
      isPinching = false;
      if (activePointers.size === 1) {
        const remaining = [...activePointers.values()][0];
        isDragging = true;
        dragStartX = remaining.x;
        dragStartY = remaining.y;
        dragStartCenter = viewCenter;
        dragStartYRatio = viewYRatio;
        pointerSamples = [{ time: performance.now(), x: remaining.x, y: remaining.y }];
      }
    }
    return;
  }

  isDragging = false;

  const elapsed = performance.now() - pointerDownTime;
  const dist = Math.hypot(e.clientX - pointerDownPos.x, e.clientY - pointerDownPos.y);
  if (elapsed < TAP_MAX_DURATION_MS && dist < TAP_MAX_DISTANCE_PX) {
    lastPointerClientX = e.clientX;
    lastPointerClientY = e.clientY;
    pointerOverCanvas = true;
    updateHover();
    pointerSamples = [];
    return;
  }

  if (pointerSamples.length >= 2) {
    const first = pointerSamples[0];
    const last = pointerSamples[pointerSamples.length - 1];
    const dt = last.time - first.time;
    if (dt > VELOCITY_MIN_DT) {
      const pxPerMsX = (last.x - first.x) / dt;
      const msPerPx = viewSpan / (width - PADDING.left - PADDING.right);
      panVelocity = -pxPerMsX * msPerPx;
      panVelocityY = (last.y - first.y) / dt;
    }
  }
  pointerSamples = [];
}

function onPointerLeave() {
  pointerOverCanvas = false;
  tooltipVisible = false;
  hoverSlotTime = null;
}

function onPointerCancel(e: PointerEvent) {
  activePointers.delete(e.pointerId);
  if (activePointers.size < 2) isPinching = false;
  if (activePointers.size === 0) isDragging = false;
}

function onWheel(e: WheelEvent) {
  e.preventDefault();
  if (!canvasEl) return;
  followEnabled = false;
  panVelocity = 0;
  panVelocityY = 0;

  const rect = canvasEl.getBoundingClientRect();
  const mouseX = e.clientX - rect.left;
  const chartWidth = width - PADDING.left - PADDING.right;

  zoomAnchorTime = xScale.invert(mouseX).getTime();
  zoomAnchorRatio = (mouseX - PADDING.left) / chartWidth;

  const zoomIn = e.deltaY < 0;
  const factor = zoomIn ? 1 / ZOOM_FACTOR : ZOOM_FACTOR;
  targetSpan = Math.min(MAX_SPAN_MS, Math.max(MIN_SPAN_MS, targetSpan * factor));
  isZoomAnimating = true;
}

function onKeyDown(e: KeyboardEvent) {
  const wasCtrl = ctrlHeld;
  ctrlHeld = e.ctrlKey || e.metaKey;
  if (ctrlHeld !== wasCtrl) updateHover();

  switch (e.key) {
    case "ArrowLeft":
    case "ArrowRight": {
      e.preventDefault();
      followEnabled = false;
      panVelocity = 0;
      const ratio = e.ctrlKey ? PAN_STEP_CTRL_RATIO : PAN_STEP_RATIO;
      const step = viewSpan * ratio;
      if (!isPanAnimating) targetCenter = viewCenter;
      targetCenter += e.key === "ArrowRight" ? step : -step;
      isPanAnimating = true;
      break;
    }
    case "ArrowUp":
    case "ArrowDown": {
      e.preventDefault();
      if (e.ctrlKey || e.metaKey) {
        const direction = e.key === "ArrowUp" ? -1 : 1;
        if (!isYPanAnimating) targetYRatio = viewYRatio;
        targetYRatio = Math.max(
          YRATIO_MIN,
          Math.min(YRATIO_MAX, targetYRatio + direction * YRATIO_STEP)
        );
        isYPanAnimating = true;
      } else {
        const factor = e.key === "ArrowUp" ? 1 / ZOOM_KEY_FACTOR : ZOOM_KEY_FACTOR;
        followEnabled = false;
        panVelocity = 0;
        zoomAnchorTime = isPanAnimating ? targetCenter : viewCenter;
        zoomAnchorRatio = 0.5;
        targetSpan = Math.min(MAX_SPAN_MS, Math.max(MIN_SPAN_MS, targetSpan * factor));
        isZoomAnimating = true;
      }
      break;
    }
  }
}

function onKeyUp(e: KeyboardEvent) {
  const wasCtrl = ctrlHeld;
  ctrlHeld = e.ctrlKey || e.metaKey;
  if (ctrlHeld !== wasCtrl) updateHover();
}

function onWindowBlur() {
  const wasCtrl = ctrlHeld;
  ctrlHeld = false;
  if (wasCtrl) updateHover();
}

function resumeFollow() {
  panVelocity = 0;
  panVelocityY = 0;
  isPanAnimating = false;
  isYPanAnimating = false;
  targetYRatio = DEFAULT_AXIS_RATIO;
  viewYRatio = DEFAULT_AXIS_RATIO;
  targetSpan = DEFAULT_SPAN_MS;
  isZoomAnimating = true;
  zoomAnchorTime = Date.now();
  zoomAnchorRatio = 0.5;
  followEnabled = true;
}

function updateSize() {
  if (!containerEl) return;
  const rect = containerEl.getBoundingClientRect();
  width = rect.width;
  height = rect.height;
  dpr = window.devicePixelRatio || 1;
  if (canvasEl) {
    canvasEl.width = width * dpr;
    canvasEl.height = height * dpr;
  }
}

function tick(timestamp: number) {
  const dt = lastFrameTime > 0 ? Math.min(timestamp - lastFrameTime, MAX_DT) : DEFAULT_DT;
  lastFrameTime = timestamp;

  const friction = Math.pow(PAN_FRICTION, dt / 16);

  if (
    !isDragging &&
    (Math.abs(panVelocity) > PAN_STOP_THRESHOLD || Math.abs(panVelocityY) > PAN_STOP_THRESHOLD_Y)
  ) {
    viewCenter += panVelocity * dt;
    viewYRatio += (panVelocityY * dt) / height;
    panVelocity *= friction;
    panVelocityY *= friction;
    if (Math.abs(panVelocity) < PAN_STOP_THRESHOLD) panVelocity = 0;
    if (Math.abs(panVelocityY) < PAN_STOP_THRESHOLD_Y) panVelocityY = 0;
  }

  if (isZoomAnimating && !isDragging) {
    const spanDiff = targetSpan - viewSpan;
    if (Math.abs(spanDiff) < ZOOM_SETTLE_THRESHOLD) {
      viewSpan = targetSpan;
      viewCenter = zoomAnchorTime + (0.5 - zoomAnchorRatio) * viewSpan;
      isZoomAnimating = false;
    } else {
      const zf = 1 - Math.pow(1 - ZOOM_EASE, dt / 16);
      viewSpan += spanDiff * zf;
      viewCenter = zoomAnchorTime + (0.5 - zoomAnchorRatio) * viewSpan;
    }
  }

  if (isPanAnimating && !isDragging) {
    const panDiff = targetCenter - viewCenter;
    const msPerPx = viewSpan / (width - PADDING.left - PADDING.right);
    if (Math.abs(panDiff) < msPerPx * PAN_SETTLE_THRESHOLD_PX) {
      viewCenter = targetCenter;
      isPanAnimating = false;
    } else {
      viewCenter += panDiff * (1 - Math.pow(1 - PAN_EASE, dt / 16));
    }
  }

  if (isYPanAnimating && !isDragging) {
    const yDiff = targetYRatio - viewYRatio;
    if (Math.abs(yDiff) < YRATIO_SETTLE_THRESHOLD) {
      viewYRatio = targetYRatio;
      isYPanAnimating = false;
    } else {
      viewYRatio += yDiff * (1 - Math.pow(1 - PAN_EASE, dt / 16));
    }
  }

  if (followEnabled && !isDragging) {
    const target = Date.now();
    viewCenter += (target - viewCenter) * (1 - Math.pow(1 - FOLLOW_EASE, dt / 16));
  }

  const result = stepAnimations(animMap, dt, animatedMaxY);
  animatedMaxY = result.maxY;
  pruneAnimMap(animMap, viewStart, viewEnd, viewSpan);

  render();
  animationFrameId = requestAnimationFrame(tick);
}

$effect(() => {
  const slots = data;
  const subs = allSubjects;
  const enabled = enabledSubjects;
  syncAnimTargets(animMap, slots, subs, enabled);
});

$effect(() => {
  store.requestRange(viewStart, viewEnd);
});

onMount(() => {
  updateSize();

  const ro = new ResizeObserver(updateSize);
  if (containerEl) ro.observe(containerEl);

  window.addEventListener("blur", onWindowBlur);

  viewCenter = Date.now();
  targetCenter = viewCenter;
  targetSpan = viewSpan;
  canvasEl?.focus();
  animationFrameId = requestAnimationFrame(tick);

  return () => {
    cancelAnimationFrame(animationFrameId);
    ro.disconnect();
    window.removeEventListener("blur", onWindowBlur);
    store.dispose();
  };
});
</script>

<div class="absolute inset-0 select-none" bind:this={containerEl}>
    <canvas
        bind:this={canvasEl}
        class="w-full h-full cursor-grab outline-none"
        class:cursor-grabbing={isDragging}
        style="display: block; touch-action: none;"
        tabindex="0"
        aria-label="Interactive enrollment timeline chart"
        onpointerdown={(e) => {
            canvasEl?.focus();
            onPointerDown(e);
        }}
        onpointermove={onPointerMove}
        onpointerup={onPointerUp}
        onpointerleave={onPointerLeave}
        onpointercancel={onPointerCancel}
        onwheel={onWheel}
        onkeydown={onKeyDown}
        onkeyup={onKeyUp}
    ></canvas>

    <TimelineDrawer
        bind:open={drawerOpen}
        subjects={allSubjects}
        {enabledSubjects}
        {followEnabled}
        onToggleSubject={toggleSubject}
        onEnableAll={enableAll}
        onDisableAll={disableAll}
        onResumeFollow={resumeFollow}
    />

    <TimelineTooltip visible={tooltipVisible} x={tooltipX} y={tooltipY} slot={tooltipSlot} {activeSubjects} />
</div>
