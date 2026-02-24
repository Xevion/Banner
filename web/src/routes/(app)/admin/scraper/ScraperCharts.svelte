<script lang="ts">
import type { ScraperPeriod } from "$lib/api";
import type { TimeseriesPoint } from "$lib/bindings";
import { mergeByKey } from "$lib/composables/reducers";
import { useStream } from "$lib/composables/useStream.svelte";
import { scaleLinear, scaleTime } from "d3-scale";
import { curveMonotoneX } from "d3-shape";
import { Area, Axis, Chart, Highlight, Svg, Tooltip } from "layerchart";
import { untrack } from "svelte";
import { cubicOut } from "svelte/easing";
import { Tween } from "svelte/motion";

interface Props {
  period: ScraperPeriod;
  term?: string;
}

let { period, term }: Props = $props();

interface TimeseriesState {
  points: TimeseriesPoint[];
  period: string;
  bucket: string;
}

const timeseries = useStream(
  "scraperTimeseries",
  untrack(() => ({ period, term })),
  {
    initial: { points: [], period: "", bucket: "" } as TimeseriesState,
    onSnapshot: (s) => ({ points: s.points, period: s.period, bucket: s.bucket }),
    onDelta: (state, delta) => ({
      ...state,
      points: mergeByKey(state.points, delta.changed, (p) => p.timestamp),
    }),
  }
);

// Reactively update filter when period/term changes
$effect(() => {
  timeseries.modify({ period, term });
});

interface ChartPoint {
  date: Date;
  success: number;
  errors: number;
  coursesChanged: number;
}

let chartData = $derived(
  (timeseries.state.points ?? []).map((p) => ({
    date: new Date(p.timestamp),
    success: p.successCount,
    errors: p.errorCount,
    coursesChanged: p.coursesChanged,
  }))
);

// Tween the data array so stacked areas stay aligned
const tweenedChart = new Tween<ChartPoint[]>([], {
  duration: 600,
  easing: cubicOut,
  interpolate(from, to) {
    if (from.length !== to.length) return () => to;
    return (t) =>
      to.map((dest, i) => ({
        date: dest.date,
        success: from[i].success + (dest.success - from[i].success) * t,
        errors: from[i].errors + (dest.errors - from[i].errors) * t,
        coursesChanged: from[i].coursesChanged + (dest.coursesChanged - from[i].coursesChanged) * t,
      }));
  },
});

$effect(() => {
  void tweenedChart.set(chartData);
});

let scrapeYMax = $derived(Math.max(1, ...chartData.map((d) => d.success + d.errors)));
let changesYMax = $derived(Math.max(1, ...chartData.map((d) => d.coursesChanged)));

function xAxisFormat(p: ScraperPeriod) {
  return (v: Date) => {
    if (p === "1h" || p === "6h") {
      return v.toLocaleTimeString("en-US", { hour: "numeric", minute: "2-digit" });
    }
    if (p === "24h") {
      return v.toLocaleTimeString("en-US", { hour: "numeric" });
    }
    return v.toLocaleDateString("en-US", { month: "short", day: "numeric" });
  };
}
</script>

{#if chartData.length > 0}
  <div class="bg-card border-border rounded-lg border p-4">
    <h2 class="mb-3 text-xs font-semibold text-foreground">Scrape Activity</h2>
    <div class="h-[250px]">
      <Chart
        data={tweenedChart.current}
        x="date"
        xScale={scaleTime()}
        y={(d: ChartPoint) => d.success + d.errors}
        yScale={scaleLinear()}
        yDomain={[0, scrapeYMax]}
        yNice
        padding={{ top: 10, bottom: 30, left: 45, right: 10 }}
        tooltip={{ mode: "bisect-x" }}
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
            format={xAxisFormat(period)}
            grid={{ class: "stroke-muted-foreground/10" }}
            rule={false}
            classes={{ tickLabel: "fill-muted-foreground" }}
          />
          <Area
            y1="success"
            fill="var(--status-green)"
            fillOpacity={0.4}
            curve={curveMonotoneX}
          />
          <Area
            y0="success"
            y1={(d: ChartPoint) => d.success + d.errors}
            fill="var(--status-red)"
            fillOpacity={0.4}
            curve={curveMonotoneX}
          />
          <Highlight lines />
        </Svg>
        <Tooltip.Root
          let:data
          classes={{ root: "text-xs" }}
          variant="none"
        >
          {@const d = data as ChartPoint}
          <div class="bg-card text-card-foreground shadow-md rounded-md px-2.5 py-1.5 flex flex-col gap-y-1">
            <p class="text-muted-foreground font-medium">{d.date.toLocaleTimeString("en-US", { hour: "numeric", minute: "2-digit" })}</p>
            <div class="flex items-center justify-between gap-4">
              <span class="flex items-center gap-1.5"><span class="inline-block size-2 rounded-full bg-status-green"></span>Successful</span>
              <span class="tabular-nums font-medium">{d.success}</span>
            </div>
            <div class="flex items-center justify-between gap-4">
              <span class="flex items-center gap-1.5"><span class="inline-block size-2 rounded-full bg-status-red"></span>Errors</span>
              <span class="tabular-nums font-medium">{d.errors}</span>
            </div>
          </div>
        </Tooltip.Root>
      </Chart>
    </div>

    <h2 class="mt-4 mb-3 text-xs font-semibold text-foreground">Courses Changed</h2>
    <div class="h-[150px]">
      <Chart
        data={tweenedChart.current}
        x="date"
        xScale={scaleTime()}
        y="coursesChanged"
        yScale={scaleLinear()}
        yDomain={[0, changesYMax]}
        yNice
        padding={{ top: 10, bottom: 30, left: 45, right: 10 }}
        tooltip={{ mode: "bisect-x" }}
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
            format={xAxisFormat(period)}
            grid={{ class: "stroke-muted-foreground/10" }}
            rule={false}
            classes={{ tickLabel: "fill-muted-foreground" }}
          />
          <Area
            fill="var(--status-blue)"
            fillOpacity={0.3}
            curve={curveMonotoneX}
          />
          <Highlight lines />
        </Svg>
        <Tooltip.Root
          let:data
          classes={{ root: "text-xs" }}
          variant="none"
        >
          {@const d = data as ChartPoint}
          <div class="bg-card text-card-foreground shadow-md rounded-md px-2.5 py-1.5 flex flex-col gap-y-1">
            <p class="text-muted-foreground font-medium">{d.date.toLocaleTimeString("en-US", { hour: "numeric", minute: "2-digit" })}</p>
            <div class="flex items-center justify-between gap-4">
              <span class="flex items-center gap-1.5"><span class="inline-block size-2 rounded-full bg-status-blue"></span>Changed</span>
              <span class="tabular-nums font-medium">{d.coursesChanged}</span>
            </div>
          </div>
        </Tooltip.Root>
      </Chart>
    </div>
  </div>
{:else}
  <div class="bg-card border-border rounded-lg border p-8 text-center text-muted-foreground">
    No chart data available for this period.
  </div>
{/if}
