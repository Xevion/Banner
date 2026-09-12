<script lang="ts">
import { type HistoryPoint, hasWaitlist, historyScaleMax } from "$lib/enrollment-history";
import { formatNumber } from "$lib/utils";
import { scaleLinear, scaleTime } from "d3-scale";
import { curveStepAfter } from "d3-shape";
import { Area, Axis, Chart, Highlight, Spline, Svg, Tooltip } from "layerchart";

let { points, height = 240 }: { points: HistoryPoint[]; height?: number } = $props();

// Snapshots are change events, not samples -- the value held until the next one, so the
// mark steps. A smoothed curve would draw enrollments that never existed.
const curve = curveStepAfter;

let yMax = $derived(Math.ceil(historyScaleMax(points) * 1.05));
let showWaitlist = $derived(hasWaitlist(points));

function formatTick(d: Date): string {
  return d.toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

function formatStamp(d: Date): string {
  return (
    d.toLocaleDateString("en-US", { month: "short", day: "numeric" }) +
    ", " +
    d.toLocaleTimeString("en-US", { hour: "numeric", minute: "2-digit" })
  );
}
</script>

<div style="height: {height}px">
  <Chart
    data={points}
    x="date"
    xScale={scaleTime()}
    y="capacity"
    yScale={scaleLinear()}
    yDomain={[0, yMax]}
    yNice
    padding={{ top: 12, bottom: 28, left: 42, right: 12 }}
    tooltipContext={{ mode: "bisect-x" }}
  >
    <Svg>
      <Axis
        placement="left"
        grid={{ class: "stroke-muted-foreground/15" }}
        rule={false}
        classes={{ tickLabel: "fill-muted-foreground" }}
      />
      <Axis
        placement="bottom"
        format={formatTick}
        ticks={6}
        grid={{ class: "stroke-muted-foreground/10" }}
        rule={false}
        classes={{ tickLabel: "fill-muted-foreground" }}
      />

      <Area y1="enrolled" fill="var(--status-blue)" fillOpacity={0.16} {curve} />

      <Spline
        y="capacity"
        stroke="var(--muted-foreground)"
        stroke-width={1.5}
        stroke-dasharray="5 4"
        {curve}
      />
      <Spline y="enrolled" stroke="var(--status-blue)" stroke-width={2} {curve} />
      {#if showWaitlist}
        <Spline y="waiting" stroke="var(--status-orange)" stroke-width={2} {curve} />
      {/if}

      <Highlight lines />
    </Svg>

    <Tooltip.Root classes={{ root: "text-xs" }} variant="none">
      {#snippet children({ data })}
        {@const d = data as HistoryPoint}
        <div
          class="bg-card text-card-foreground shadow-md rounded-md px-2.5 py-1.5 flex flex-col gap-y-1 border border-border"
        >
          <p class="text-muted-foreground font-medium">{formatStamp(d.date)}</p>
          <div class="flex items-center justify-between gap-4">
            <span class="flex items-center gap-1.5">
              <span class="inline-block size-2 rounded-full bg-status-blue"></span>Enrolled
            </span>
            <span class="tabular-nums font-medium"
              >{formatNumber(d.enrolled)} / {formatNumber(d.capacity)}</span
            >
          </div>
          <div class="flex items-center justify-between gap-4">
            <span class="text-muted-foreground">Open</span>
            <span class="tabular-nums font-medium">{formatNumber(d.open)}</span>
          </div>
          {#if showWaitlist}
            <div class="flex items-center justify-between gap-4">
              <span class="flex items-center gap-1.5">
                <span class="inline-block size-2 rounded-full bg-status-orange"></span>Waiting
              </span>
              <span class="tabular-nums font-medium">{formatNumber(d.waiting)}</span>
            </div>
          {/if}
        </div>
      {/snippet}
    </Tooltip.Root>
  </Chart>
</div>
