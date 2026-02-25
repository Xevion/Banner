<script lang="ts">
import { ratingColor } from "$lib/course";
import { themeStore } from "$lib/stores/theme.svelte";
import { cn } from "$lib/utils";
import { scaleBand, scaleLinear } from "d3-scale";
import { Axis, Bars, Chart, Highlight, Points, Rule, Svg, Tooltip } from "layerchart";

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
const rmpColor = $derived(rmpRating != null ? ratingColor(rmpRating, themeStore.isDark) : null);
const bbColor = $derived(bbRating != null ? ratingColor(bbRating, themeStore.isDark) : null);

// Unique filter ID to avoid collisions when multiple ScoreBars render
const filterId = `dot-glow-${Math.random().toString(36).slice(2, 8)}`;

const sourceLabel = $derived.by(() => {
  if (source === "both") return "RateMyProfessors + BlueBook";
  if (source === "rmp") return "RateMyProfessors";
  return "BlueBook";
});
</script>

<div class={cn("flex w-full items-center gap-3 min-h-11", className)}>
    <!-- Score number + CI range -->
    <div class="w-12 shrink-0 text-right">
        <span class="text-sm font-semibold tabular-nums leading-tight" style="color: {color}">
            {displayScore.toFixed(1)}
        </span>
        <div class="text-[10px] text-muted-foreground tabular-nums leading-tight">
            {ciLower.toFixed(1)}–{ciUpper.toFixed(1)}
        </div>
    </div>

    <!-- Chart -->
    <div class="h-18 flex-1">
        <Chart
            data={chartData}
            x="ciLower"
            x1="ciUpper"
            xScale={scaleLinear()}
            xDomain={[SCALE_MIN, SCALE_MAX]}
            y="label"
            yScale={scaleBand().padding(0.35)}
            padding={{ top: 4, bottom: 20, left: 2, right: 2 }}
            tooltip={{ mode: "band" }}
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
                    rule={false}
                    grid={false}
                    classes={{
                        tickLabel: "fill-muted-foreground text-[10px]",
                        tick: "stroke-border/60",
                    }}
                    tickLength={3}
                />

                <!-- Prior mean reference line -->
                <Rule
                    x={PRIOR_MEAN}
                    class="stroke-muted-foreground/25"
                    stroke-dasharray="3 2"
                    strokeWidth={1}
                />

                <!-- CI range band with confidence-modulated opacity -->
                <Bars
                    radius={3}
                    fill={color}
                    fillOpacity={ciOpacity}
                    stroke={color}
                    strokeWidth={1}
                    strokeOpacity={0.35}
                />

                <!-- Source markers (only when both sources present) -->
                {#if source === "both" && rmpRating != null}
                    <Points x="rmpRating" r={3} let:points>
                        {#each points as point (point.xValue)}
                            <polygon
                                points="{point.x},{point.y - 7} {point.x - 3.5},{point.y - 1} {point.x +
                                    3.5},{point.y - 1}"
                                fill={rmpColor}
                                fill-opacity="0.6"
                            />
                        {/each}
                    </Points>
                {/if}

                {#if source === "both" && bbRating != null}
                    <Points x="bbRating" r={3} let:points>
                        {#each points as point (point.xValue)}
                            <polygon
                                points="{point.x},{point.y + 1} {point.x - 3.5},{point.y + 7} {point.x +
                                    3.5},{point.y + 7}"
                                fill={bbColor}
                                fill-opacity="0.6"
                            />
                        {/each}
                    </Points>
                {/if}

                <!-- Point estimate dot with glow -->
                <g filter="url(#{filterId})">
                    <Points x="displayScore" r={5.5} fill={color} />
                </g>

                <Highlight bar={{ class: "fill-foreground/5", stroke: "none" }} />
            </Svg>

            <Tooltip.Root
                let:data
                variant="none"
                contained="window"
                classes={{ root: "z-50 pointer-events-none" }}
            >
                {@const d = data as DataPoint}
                {@const tipColor = ratingColor(d.displayScore, themeStore.isDark)}
                <div
                    class="bg-card text-card-foreground border border-border flex min-w-55 flex-col gap-y-1.5 rounded-md px-2.5 py-2 text-xs shadow-md"
                >
                    <!-- Score -->
                    <div class="flex items-center justify-between gap-4">
                        <span class="font-medium">Score</span>
                        <span class="font-semibold tabular-nums" style="color: {tipColor}">
                            {d.displayScore.toFixed(2)}
                        </span>
                    </div>

                    <!-- Confidence with visual bar -->
                    <div class="flex items-center justify-between gap-4">
                        <span class="text-muted-foreground">Confidence</span>
                        <div class="flex items-center gap-1.5">
                            <div class="h-1.5 w-16 rounded-full bg-muted overflow-hidden">
                                <div
                                    class="h-full rounded-full"
                                    style="width: {d.confidence * 100}%; background: {tipColor}"
                                ></div>
                            </div>
                            <span class="tabular-nums text-muted-foreground w-8 text-right">
                                {(d.confidence * 100).toFixed(0)}%
                            </span>
                        </div>
                    </div>

                    <!-- Range -->
                    <div class="flex items-center justify-between gap-4">
                        <span class="text-muted-foreground">Range</span>
                        <span class="tabular-nums">
                            {d.ciLower.toFixed(2)} – {d.ciUpper.toFixed(2)}
                        </span>
                    </div>

                    <hr class="border-border" />

                    <!-- Source -->
                    <div class="flex items-center justify-between gap-4">
                        <span class="text-muted-foreground">Source</span>
                        <span class="text-right">{sourceLabel}</span>
                    </div>

                    {#if d.rmpRating != null}
                        <div class="flex items-center justify-between gap-4">
                            <span class="flex items-center gap-1.5">
                                {#if d.source === "both"}<span class="text-[8px] leading-none">▲</span>{/if}
                                <span class="text-muted-foreground">RateMyProfessors</span>
                            </span>
                            <span class="tabular-nums">
                                {d.rmpRating.toFixed(1)}
                                <span class="text-muted-foreground">({d.rmpCount})</span>
                            </span>
                        </div>
                    {/if}

                    {#if d.bbRating != null}
                        <div class="flex items-center justify-between gap-4">
                            <span class="flex items-center gap-1.5">
                                {#if d.source === "both"}<span class="text-[8px] leading-none">▼</span>{/if}
                                <span class="text-muted-foreground">BlueBook</span>
                            </span>
                            <span class="tabular-nums">
                                {d.bbRating.toFixed(2)}
                                <span class="text-muted-foreground">({d.bbCount})</span>
                            </span>
                        </div>
                    {/if}

                    <hr class="border-border" />

                    <!-- Prior mean reference -->
                    <div class="flex items-center justify-between gap-4">
                        <span class="text-muted-foreground">Avg (all profs)</span>
                        <span class="tabular-nums">{PRIOR_MEAN.toFixed(2)}</span>
                    </div>
                </div>
            </Tooltip.Root>
        </Chart>
    </div>
</div>
