<script lang="ts">
import { ratingColor } from "$lib/course";
import { themeStore } from "$lib/stores/theme.svelte";
import { cn } from "$lib/utils";
import { scaleBand, scaleLinear } from "d3-scale";
import { Axis, Bars, Chart, Points, Rule, Svg } from "layerchart";

let {
  displayScore,
  sortScore,
  ciLower,
  ciUpper,
  confidence,
  source,
  rmpRating = null,
  rmpCount = 0,
  bbRating = null,
  bbCount = 0,
  class: className = "",
}: {
  displayScore: number;
  sortScore: number;
  ciLower: number;
  ciUpper: number;
  confidence: number;
  source: "both" | "rmp" | "bb";
  rmpRating?: number | null;
  rmpCount?: number;
  bbRating?: number | null;
  bbCount?: number;
  class?: string;
} = $props();

const SCALE_MIN = 1.0;
const SCALE_MAX = 5.0;
const PRIOR_MEAN = 3.775;
const PADDING = { top: 4, bottom: 20, left: 2, right: 2 };
const SYMBOL_THRESHOLD = 30;

interface DataPoint {
  label: string;
  displayScore: number;
  sortScore: number;
  ciLower: number;
  ciUpper: number;
  confidence: number;
  source: "both" | "rmp" | "bb";
  rmpRating: number | null;
  rmpCount: number;
  bbRating: number | null;
  bbCount: number;
}

const chartData: DataPoint[] = $derived([
  {
    label: "",
    displayScore,
    sortScore,
    ciLower,
    ciUpper,
    confidence,
    source,
    rmpRating,
    rmpCount,
    bbRating,
    bbCount,
  },
]);

const color = $derived(ratingColor(displayScore, themeStore.isDark));
const ciOpacity = $derived(0.12 + confidence * 0.25);
const rmpColor = $derived(rmpRating != null ? "#f59e0b" : null); // amber - independent of rating gradient
const bbColor = $derived(bbRating != null ? "#60a5fa" : null); // blue - independent of rating gradient

const filterId = `dot-glow-${Math.random().toString(36).slice(2, 8)}`;

// Container dimensions for custom tooltip positioning
let containerWidth = $state(0);
let containerHeight = $state(0);
let chartContainer = $state<HTMLElement | null>(null);

// Build a local xScale that mirrors what Chart creates internally
const xScale = $derived(
  scaleLinear()
    .domain([SCALE_MIN, SCALE_MAX])
    .range([PADDING.left, containerWidth - PADDING.right])
);

const bandCenter = $derived((containerHeight - PADDING.bottom - PADDING.top) / 2 + PADDING.top);

// Symbol definitions for hit-testing
interface SymbolDef {
  id: string;
  x: number;
  y: number;
  label: string;
  value: string;
  color: string;
  icon: string;
  detail?: string;
}

const symbols: SymbolDef[] = $derived.by(() => {
  if (containerWidth === 0) return [];
  const syms: SymbolDef[] = [];

  syms.push({
    id: "score",
    x: xScale(displayScore),
    y: bandCenter,
    label: "Rating",
    value: displayScore.toFixed(2),
    color,
    icon: "\u25CF",
    detail: `${(confidence * 100).toFixed(0)}% confidence \u00B7 ${ciLower.toFixed(2)}\u2013${ciUpper.toFixed(2)} range`,
  });

  if (source === "both" && rmpRating != null && rmpColor) {
    syms.push({
      id: "rmp",
      x: xScale(rmpRating),
      y: bandCenter - 4,
      label: "RateMyProfessors",
      value: rmpRating.toFixed(1),
      color: rmpColor,
      icon: "\u2605",
    });
  }

  if (source === "both" && bbRating != null && bbColor) {
    syms.push({
      id: "bb",
      x: xScale(bbRating),
      y: bandCenter + 4,
      label: "BlueBook",
      value: bbRating.toFixed(2),
      color: bbColor,
      icon: "\u25B2",
    });
  }

  syms.push({
    id: "prior",
    x: xScale(PRIOR_MEAN),
    y: bandCenter,
    label: "Avg (all instructors)",
    value: PRIOR_MEAN.toFixed(2),
    color: "var(--color-muted-foreground)",
    icon: "\u250A",
  });

  return syms;
});

// Tooltip state
let mouseX = $state(0);
let mouseY = $state(0);
let isHovering = $state(false);

const activeSymbol: SymbolDef | null = $derived.by(() => {
  if (!isHovering || symbols.length === 0) return null;

  let closest: SymbolDef | null = null;
  let closestDist = SYMBOL_THRESHOLD;

  for (const sym of symbols) {
    const dx = mouseX - sym.x;
    const dy = mouseY - sym.y;
    const dist = Math.sqrt(dx * dx + dy * dy);
    if (dist < closestDist) {
      closest = sym;
      closestDist = dist;
    }
  }

  return closest;
});

// When active symbol exists, tooltip anchors to symbol; otherwise follows cursor
const showMainTooltip = $derived(isHovering && !activeSymbol);
const showSymbolTooltip = $derived(isHovering && activeSymbol != null);

// Dim everything except the highlighted symbol
const DIM = 0.18;
const highlightId = $derived(activeSymbol?.id ?? null);
const bandDimOpacity = $derived(highlightId != null && highlightId !== "score" ? DIM : 1);
const dotDimOpacity = $derived(highlightId != null && highlightId !== "score" ? DIM : 1);
const rmpDimOpacity = $derived(highlightId != null && highlightId !== "rmp" ? DIM : 1);
const bbDimOpacity = $derived(highlightId != null && highlightId !== "bb" ? DIM : 1);
const priorDimOpacity = $derived(highlightId != null && highlightId !== "prior" ? DIM : 1);

function onMouseMove(e: MouseEvent) {
  const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
  mouseX = e.clientX - rect.left;
  mouseY = e.clientY - rect.top;
  isHovering = true;
}

function onMouseLeave() {
  isHovering = false;
}

function onTouchStart(e: TouchEvent) {
  const touch = e.touches[0];
  const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
  mouseX = touch.clientX - rect.left;
  mouseY = touch.clientY - rect.top;
  isHovering = true;
}

function onTouchEnd() {
  isHovering = false;
}

// touchmove must be registered non-passively to allow preventDefault(),
// which prevents the page from scrolling while the user drags over the chart.
$effect(() => {
  const el = chartContainer;
  if (!el) return;

  function handleTouchMove(e: TouchEvent) {
    if (!el) return;
    e.preventDefault();
    const touch = e.touches[0];
    const rect = el.getBoundingClientRect();
    mouseX = touch.clientX - rect.left;
    mouseY = touch.clientY - rect.top;
    isHovering = true;
  }

  el.addEventListener("touchmove", handleTouchMove, { passive: false });
  return () => el.removeEventListener("touchmove", handleTouchMove);
});

// CI bound tick positions (avoid overlap with integer ticks)
const ciTicks = $derived.by(() => {
  const ticks: { value: number; x: number }[] = [];
  if (containerWidth === 0) return ticks;
  const integerTicks = [1, 2, 3, 4, 5];
  for (const val of [ciLower, ciUpper]) {
    const tooClose = integerTicks.some((t) => Math.abs(val - t) < 0.15);
    if (!tooClose) {
      ticks.push({ value: val, x: xScale(val) });
    }
  }
  return ticks;
});

// Axis bottom Y position (where tick marks should be drawn)
const axisY = $derived(containerHeight - PADDING.bottom);
</script>

<div class={cn("flex w-full items-center gap-2 sm:gap-3 min-h-11", className)}>
    <!-- Score number + CI range -->
    <div class="w-10 sm:w-12 shrink-0 text-right">
        <span class="text-xl sm:text-3xl font-bold tabular-nums leading-tight" style="color: {color}">
            {displayScore.toFixed(1)}
        </span>
        <div class="text-[10px] text-muted-foreground tabular-nums leading-tight">
            {ciLower.toFixed(1)}&ndash;{ciUpper.toFixed(1)}
        </div>
    </div>

    <!-- Chart with custom tooltip overlay -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
        class="h-18 flex-1 relative"
        bind:this={chartContainer}
        bind:clientWidth={containerWidth}
        bind:clientHeight={containerHeight}
        onmousemove={onMouseMove}
        onmouseleave={onMouseLeave}
        ontouchstart={onTouchStart}
        ontouchend={onTouchEnd}
        ontouchcancel={onTouchEnd}
    >
        <Chart
            data={chartData}
            x={(d: DataPoint) => [d.ciLower, d.ciUpper]}
            xScale={scaleLinear()}
            xDomain={[SCALE_MIN, SCALE_MAX]}
            y="label"
            yScale={scaleBand().padding(0.35)}
            padding={PADDING}
        >
            <Svg>
                {#snippet defs()}
                    <filter id={filterId} x="-50%" y="-50%" width="200%" height="200%">
                        <feGaussianBlur in="SourceGraphic" stdDeviation="2" result="blur" />
                        <feMerge>
                            <feMergeNode in="blur" />
                            <feMergeNode in="SourceGraphic" />
                        </feMerge>
                    </filter>
                {/snippet}

                <Axis
                    placement="bottom"
                    ticks={[1, 2, 3, 4, 5]}
                    rule
                    grid={false}
                    classes={{
                        tickLabel: "fill-foreground/65 text-[10px]",
                        tick: "stroke-foreground/30",
                        rule: "stroke-foreground/20",
                    }}
                    tickLength={3}
                />

                <!-- CI bound tick marks on axis -->
                {#each ciTicks as tick (tick.value)}
                    <line
                        x1={tick.x}
                        y1={axisY}
                        x2={tick.x}
                        y2={axisY + 4}
                        class="stroke-foreground/65"
                        stroke-width="1"
                    />
                    <text
                        x={tick.x}
                        y={axisY + 13}
                        text-anchor="middle"
                        class="fill-foreground/70 text-[9px] font-medium"
                    >
                        {tick.value.toFixed(1)}
                    </text>
                {/each}

                <!-- Prior mean reference line -->
                <g opacity={priorDimOpacity} style="transition: opacity 0.12s ease">
                    <Rule
                        x={PRIOR_MEAN}
                        class="stroke-muted-foreground/55"
                        stroke-dasharray="3 2"
                        strokeWidth={1}
                    />
                </g>

                <!-- CI range band -->
                <g opacity={bandDimOpacity} style="transition: opacity 0.12s ease">
                    <Bars
                        radius={3}
                        fill={color}
                        fillOpacity={ciOpacity}
                        stroke={color}
                        strokeWidth={1}
                        strokeOpacity={0.35}
                    />
                </g>

                <!-- Source markers -->
                {#if source === "both" && rmpRating != null}
                    <!-- Star [star] centered ~4px above band center; R=4.5 outer, r=1.9 inner -->
                    <g opacity={rmpDimOpacity} style="transition: opacity 0.12s ease">
                        <Points x="rmpRating" r={3} let:points>
                            {#each points as point (point.xValue)}
                                <polygon
                                    points="{point.x},{point.y - 8.5} {point.x + 1.1},{point.y - 5.5} {point.x + 4.3},{point.y - 5.4} {point.x + 1.8},{point.y - 3.4} {point.x + 2.6},{point.y - 0.4} {point.x},{point.y - 2.1} {point.x - 2.6},{point.y - 0.4} {point.x - 1.8},{point.y - 3.4} {point.x - 4.3},{point.y - 5.4} {point.x - 1.1},{point.y - 5.5}"
                                    fill={rmpColor}
                                    fill-opacity="0.85"
                                />
                            {/each}
                        </Points>
                    </g>
                {/if}

                {#if source === "both" && bbRating != null}
                    <!-- Upward triangle [triangle] below band center -->
                    <g opacity={bbDimOpacity} style="transition: opacity 0.12s ease">
                        <Points x="bbRating" r={3} let:points>
                            {#each points as point (point.xValue)}
                                <polygon
                                    points="{point.x},{point.y + 1} {point.x - 3.5},{point.y + 7} {point.x + 3.5},{point.y + 7}"
                                    fill={bbColor}
                                    fill-opacity="0.85"
                                />
                            {/each}
                        </Points>
                    </g>
                {/if}

                <!-- Point estimate dot with glow -->
                <g filter="url(#{filterId})" opacity={dotDimOpacity} style="transition: opacity 0.12s ease">
                    <Points x="displayScore" r={5.5} fill={color} />
                </g>
            </Svg>
        </Chart>

        <!-- Symbol tooltip (anchored to symbol) -->
        {#if showSymbolTooltip && activeSymbol}
            <div
                class="absolute z-50 pointer-events-none"
                style="left: {activeSymbol.x}px; top: {activeSymbol.y}px; transform: translate(-50%, -100%) translateY(-10px);"
            >
                <div
                    class="bg-card text-card-foreground border border-border rounded-md px-2 py-1.5 text-xs shadow-md whitespace-nowrap"
                >
                    <div class="flex items-center gap-1.5">
                        <span class="text-sm leading-none" style="color: {activeSymbol.color}">{activeSymbol.icon}</span>
                        <span class="text-muted-foreground">{activeSymbol.label}</span>
                        <span class="font-semibold tabular-nums" style="color: {activeSymbol.color}">
                            {activeSymbol.value}
                        </span>
                    </div>
                    {#if activeSymbol.detail}
                        <div class="text-[10px] text-muted-foreground mt-0.5">
                            {activeSymbol.detail}
                        </div>
                    {/if}
                </div>
            </div>
        {/if}

        <!-- Main tooltip (follows cursor) -->
        {#if showMainTooltip}
            <div
                class="absolute z-50 pointer-events-none"
                style="left: {mouseX}px; top: {mouseY}px; transform: translate(-50%, -100%) translateY(-12px);"
            >
                <div
                    class="bg-card text-card-foreground border border-border flex min-w-48 flex-col gap-y-1.5 rounded-md px-2.5 py-2 text-xs shadow-md"
                >
                    <div class="flex items-center justify-between gap-4">
                        <span class="font-medium">Rating</span>
                        <span class="font-semibold tabular-nums" style="color: {color}">
                            {displayScore.toFixed(2)}
                        </span>
                    </div>

                    <div class="flex items-center justify-between gap-4">
                        <span class="text-muted-foreground">Confidence</span>
                        <div class="flex items-center gap-1.5">
                            <div class="h-1.5 w-16 rounded-full bg-muted overflow-hidden">
                                <div
                                    class="h-full rounded-full"
                                    style="width: {confidence * 100}%; background: {color}"
                                ></div>
                            </div>
                            <span class="tabular-nums text-muted-foreground w-8 text-right">
                                {(confidence * 100).toFixed(0)}%
                            </span>
                        </div>
                    </div>

                    <div class="flex items-center justify-between gap-4">
                        <span class="text-muted-foreground">Range</span>
                        <span class="tabular-nums">
                            {ciLower.toFixed(2)} &ndash; {ciUpper.toFixed(2)}
                        </span>
                    </div>

                    <hr class="border-border" />

                    {#if rmpRating != null}
                        <div class="flex items-center justify-between gap-4">
                            <span class="flex items-center gap-1.5">
                                {#if source === "both"}<span class="text-[11px] leading-none" style="color: {rmpColor}">&#9733;</span>{/if}
                                <span class="text-muted-foreground">RateMyProfessors</span>
                            </span>
                            <span class="tabular-nums">
                                {rmpRating.toFixed(1)}
                            </span>
                        </div>
                    {/if}

                    {#if bbRating != null}
                        <div class="flex items-center justify-between gap-4">
                            <span class="flex items-center gap-1.5">
                                {#if source === "both"}<span class="text-[11px] leading-none" style="color: {bbColor}">&#9650;</span>{/if}
                                <span class="text-muted-foreground">BlueBook</span>
                            </span>
                            <span class="tabular-nums">
                                {bbRating.toFixed(2)}
                            </span>
                        </div>
                    {/if}

                    <hr class="border-border" />

                    <div class="flex items-center justify-between gap-4">
                        <span class="text-muted-foreground">Avg (all profs)</span>
                        <span class="tabular-nums">{PRIOR_MEAN.toFixed(2)}</span>
                    </div>
                </div>
            </div>
        {/if}
    </div>
</div>
