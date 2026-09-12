<script lang="ts">
import { BannerApiClient } from "$lib/api";
import type { CourseResponse } from "$lib/bindings";
import { CHART_MIN_POINTS, type HistoryPoint, toHistoryPoints } from "$lib/enrollment-history";
import { formatNumber } from "$lib/utils";
import { BarChart3, List } from "@lucide/svelte";
import EnrollmentStepChart from "./EnrollmentStepChart.svelte";
import EnrollmentTimeline from "./EnrollmentTimeline.svelte";

let { course }: { course: CourseResponse } = $props();

type View = "chart" | "timeline";

let points = $state<HistoryPoint[]>([]);
let loading = $state(true);
let failed = $state(false);
let view = $state<View>("chart");
let spacing = $state<"change" | "day">("change");

$effect(() => {
  const term = course.termSlug;
  const crn = course.crn;
  let cancelled = false;

  loading = true;
  failed = false;

  const client = new BannerApiClient();
  void client.getMetrics({ term, crn, range: "term", limit: 1000 }).then((result) => {
    if (cancelled) return;
    result.match({
      Ok: (data) => {
        points = toHistoryPoints(data.metrics);
        // A section nobody touched still has its first sighting on record; one point is
        // not a history, so it reads as empty rather than as a single flat bar.
        if (points.length >= CHART_MIN_POINTS) view = "chart";
        else view = "timeline";
      },
      Err: () => {
        failed = true;
      },
    });
    loading = false;
  });

  return () => {
    cancelled = true;
  };
});

let canChart = $derived(points.length >= CHART_MIN_POINTS);

const toggleClass =
  "inline-flex items-center gap-1.5 rounded-md border px-2 py-1 font-mono text-[11px] transition-colors cursor-pointer";
</script>

<div class="flex flex-col gap-3 p-4">
  {#if loading}
    <p class="py-8 text-center text-sm text-muted-foreground">Loading enrollment history&hellip;</p>
  {:else if failed}
    <p class="py-8 text-center text-sm text-muted-foreground">
      Enrollment history is unavailable right now.
    </p>
  {:else if points.length < 2}
    <div class="flex flex-col items-center justify-center gap-1 py-8 text-center">
      <p class="text-sm text-muted-foreground">No enrollment changes recorded yet.</p>
      <p class="text-xs text-muted-foreground/60">
        History appears once seats or the waitlist start moving.
      </p>
    </div>
  {:else}
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div class="flex flex-col">
        <span class="text-xs font-medium text-foreground">
          {formatNumber(points.length)} recorded {points.length === 1 ? "change" : "changes"}
        </span>
        <span class="font-mono text-[11px] text-muted-foreground">
          since {points[0].date.toLocaleDateString("en-US", { month: "short", day: "numeric" })}
        </span>
      </div>

      <div class="flex gap-1.5">
        {#if canChart}
          <button
            type="button"
            class="{toggleClass} {view === 'chart'
              ? 'border-foreground/30 bg-muted text-foreground'
              : 'border-border text-muted-foreground hover:text-foreground'}"
            aria-pressed={view === "chart"}
            onclick={() => (view = "chart")}
          >
            <BarChart3 class="size-3" />
            Chart
          </button>
        {/if}
        <button
          type="button"
          class="{toggleClass} {view === 'timeline'
            ? 'border-foreground/30 bg-muted text-foreground'
            : 'border-border text-muted-foreground hover:text-foreground'}"
          aria-pressed={view === "timeline"}
          onclick={() => (view = "timeline")}
        >
          <List class="size-3" />
          Timeline
        </button>
      </div>
    </div>

    {#if view === "chart"}
      <div class="flex gap-4 font-mono text-[11px] text-muted-foreground">
        <span class="inline-flex items-center gap-1.5">
          <span class="inline-block size-2 rounded-full bg-status-blue"></span>Enrolled
        </span>
        <span class="inline-flex items-center gap-1.5">
          <span class="inline-block size-2 rounded-full bg-status-orange"></span>Waiting
        </span>
        <span class="inline-flex items-center gap-1.5">
          <span class="inline-block w-3 border-t border-dashed border-muted-foreground"></span
          >Capacity
        </span>
      </div>
      <EnrollmentStepChart {points} />
    {:else}
      <div class="flex gap-1.5">
        <button
          type="button"
          class="{toggleClass} {spacing === 'change'
            ? 'border-foreground/30 bg-muted text-foreground'
            : 'border-border text-muted-foreground hover:text-foreground'}"
          aria-pressed={spacing === "change"}
          onclick={() => (spacing = "change")}
        >
          Per change
        </button>
        <button
          type="button"
          class="{toggleClass} {spacing === 'day'
            ? 'border-foreground/30 bg-muted text-foreground'
            : 'border-border text-muted-foreground hover:text-foreground'}"
          aria-pressed={spacing === "day"}
          onclick={() => (spacing = "day")}
        >
          Per day
        </button>
      </div>
      <EnrollmentTimeline {points} bind:spacing />
    {/if}
  {/if}
</div>
