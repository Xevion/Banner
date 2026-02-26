<script lang="ts">
import type { ScraperPeriod } from "$lib/api";
import type { ScraperStatsResponse, SubjectSummary } from "$lib/bindings";
import SimpleTooltip from "$lib/components/SimpleTooltip.svelte";
import StyledSelect from "$lib/components/StyledSelect.svelte";
import { mergeByKey } from "$lib/composables/reducers";
import { useStream } from "$lib/composables/useStream.svelte";
import { formatDurationMs } from "$lib/time";
import { formatNumber } from "$lib/utils";
import { Info } from "@lucide/svelte";
import { Tabs } from "bits-ui";
import type { PageData } from "./$types";

import { untrack } from "svelte";
import ScraperAudit from "./ScraperAudit.svelte";
import ScraperCharts from "./ScraperCharts.svelte";
import ScraperJobs from "./ScraperJobs.svelte";
import ScraperSubjects from "./ScraperSubjects.svelte";

let { data }: { data: PageData } = $props();

const PERIODS: ScraperPeriod[] = ["1h", "6h", "24h", "7d", "30d"];

let selectedPeriod = $state<ScraperPeriod>("24h");
let selectedTerm = $state<string | undefined>(undefined);

// Tab state
let activeTab = $state("charts");

const stats = useStream(
  "scraperStats",
  untrack(() => ({ period: selectedPeriod, term: selectedTerm })),
  {
    initial: null as ScraperStatsResponse | null,
    onDelta: (_, delta) => delta.stats,
  }
);

// Reactively update stats filter when period/term changes
$effect(() => {
  stats.modify({ period: selectedPeriod, term: selectedTerm });
});

const subjects = useStream("scraperSubjects", null, {
  initial: [] as SubjectSummary[],
  onDelta: (state, delta) => mergeByKey(state, delta.changed, (s) => s.subject, delta.removed),
});

// Derived data with defaults
let currentStats = $derived(stats.state);
let currentSubjects = $derived(subjects.state);
let currentTerms = $derived(data.terms);

let termItems = $derived([
  { value: "", label: "All Terms" },
  ...currentTerms.map((t) => ({ value: t.code, label: t.description })),
]);

let termSelectValue = $derived(selectedTerm ?? "");

function successRateColor(rate: number): string {
  if (rate >= 0.95) return "text-green-600 dark:text-green-400";
  if (rate >= 0.8) return "text-yellow-600 dark:text-yellow-400";
  return "text-red-600 dark:text-red-400";
}
</script>

<svelte:head>
  <title>Scraper | Banner</title>
</svelte:head>

<div class="flex flex-col gap-y-6">
  <!-- Header: Title + Connection indicator + Controls -->
  <div class="grid grid-cols-1 items-center gap-x-4 gap-y-2
              sm:grid-cols-[auto_1fr]
              lg:grid-cols-[auto_auto_1fr]">
    <h1 class="text-base font-semibold text-foreground sm:col-span-2 lg:col-span-1">Scraper</h1>

    <!-- Connection indicator -->
    <div class="flex items-center">
      {#if stats.connectionState === "connected"}
        <span class="inline-flex items-center gap-1.5 text-sm">
          <span class="size-2 shrink-0 rounded-full bg-green-500"></span>
          <span class="text-green-500 lg:hidden">Live</span>
        </span>
      {:else if stats.connectionState === "reconnecting"}
        <span class="inline-flex items-center gap-1.5 text-sm">
          <span class="size-2 shrink-0 rounded-full bg-amber-500 animate-pulse"></span>
          <span class="text-amber-500">Reconnecting...</span>
        </span>
      {:else}
        <span class="inline-flex items-center gap-2 text-sm">
          <span class="inline-flex items-center gap-1.5">
            <span class="size-2 shrink-0 rounded-full bg-red-500"></span>
            <span class="text-red-500">Disconnected</span>
          </span>
          <button
            class="rounded-md bg-muted px-2 py-0.5 text-xs font-medium text-foreground hover:bg-muted/80 transition-colors"
            onclick={() => stats.retry()}
          >
            Retry
          </button>
        </span>
      {/if}
    </div>

    <div class="flex items-center gap-2 sm:justify-self-end">
      <!-- Term Dropdown -->
      <StyledSelect
        items={termItems}
        value={termSelectValue}
        onValueChange={(v) => { selectedTerm = v === "" ? undefined : v; }}
        placeholder="All Terms"
      />

      <!-- Period Selector -->
      <div class="bg-muted flex rounded-md p-0.5">
        {#each PERIODS as period (period)}
          <button
            class="rounded px-2.5 py-1 text-xs font-medium transition-colors
              {selectedPeriod === period
              ? 'bg-background text-foreground shadow-sm'
              : 'text-muted-foreground hover:text-foreground'}"
            onclick={() => (selectedPeriod = period)}
          >
            {period}
          </button>
        {/each}
      </div>
    </div>
  </div>

  {#if stats.connectionState === "disconnected" && !currentStats}
    <p class="text-destructive">WebSocket connection lost</p>
  {:else if currentStats}
    <!-- Aggregate Stats Cards -->
    <div class="grid grid-cols-2 gap-4 lg:grid-cols-4">
      <div class="bg-card border-border rounded-lg border p-3">
        <p class="text-muted-foreground text-xs">Total Scrapes</p>
        <p class="text-2xl font-bold">{formatNumber(currentStats.totalScrapes)}</p>
      </div>
      <div class="bg-card border-border rounded-lg border p-3">
        <p class="text-muted-foreground text-xs">Success Rate</p>
        {#if currentStats.successRate != null}
          <p class="text-2xl font-bold {successRateColor(currentStats.successRate)}">
            {(currentStats.successRate * 100).toFixed(1)}%
          </p>
        {:else}
          <p class="text-2xl font-bold text-muted-foreground">N/A</p>
        {/if}
      </div>
      <div class="bg-card border-border rounded-lg border p-3">
        <div class="flex items-center gap-1">
          <p class="text-muted-foreground text-xs">Avg Duration</p>
          <SimpleTooltip text="Average time per successful subject scrape (API fetch + database update)" side="top" passthrough>
            <Info class="size-3 text-muted-foreground/60" />
          </SimpleTooltip>
        </div>
        {#if currentStats.avgDurationMs != null}
          <p class="text-2xl font-bold">{formatDurationMs(currentStats.avgDurationMs)}</p>
        {:else}
          <p class="text-2xl font-bold text-muted-foreground">N/A</p>
        {/if}
      </div>
      <div class="bg-card border-border rounded-lg border p-3">
        <div class="flex items-center gap-1">
          <p class="text-muted-foreground text-xs">Courses Changed</p>
          <SimpleTooltip text="Total courses that had enrollment or schedule updates detected" side="top" passthrough>
            <Info class="size-3 text-muted-foreground/60" />
          </SimpleTooltip>
        </div>
        <p class="text-2xl font-bold">{formatNumber(currentStats.totalCoursesChanged)}</p>
      </div>
      <div class="bg-card border-border rounded-lg border p-3">
        <div class="flex items-center gap-1">
          <p class="text-muted-foreground text-xs">Pending Jobs</p>
          <SimpleTooltip text="Scrape jobs queued but not yet started (unlocked jobs waiting for a worker)" side="top" passthrough>
            <Info class="size-3 text-muted-foreground/60" />
          </SimpleTooltip>
        </div>
        <p class="text-2xl font-bold">{formatNumber(currentStats.pendingJobs)}</p>
      </div>
      <div class="bg-card border-border rounded-lg border p-3">
        <div class="flex items-center gap-1">
          <p class="text-muted-foreground text-xs">Locked Jobs</p>
          <SimpleTooltip text="Scrape jobs currently being processed by a worker" side="top" passthrough>
            <Info class="size-3 text-muted-foreground/60" />
          </SimpleTooltip>
        </div>
        <p class="text-2xl font-bold">{formatNumber(currentStats.lockedJobs)}</p>
      </div>
      <div class="bg-card border-border rounded-lg border p-3">
        <div class="flex items-center gap-1">
          <p class="text-muted-foreground text-xs">Courses Fetched</p>
          <SimpleTooltip text="Total courses retrieved from Banner API across all successful scrapes" side="top" passthrough>
            <Info class="size-3 text-muted-foreground/60" />
          </SimpleTooltip>
        </div>
        <p class="text-2xl font-bold">{formatNumber(currentStats.totalCoursesFetched)}</p>
      </div>
      <div class="bg-card border-border rounded-lg border p-3">
        <div class="flex items-center gap-1">
          <p class="text-muted-foreground text-xs">Audits Generated</p>
          <SimpleTooltip text="Change records created when course enrollment or schedule data changes" side="top" passthrough>
            <Info class="size-3 text-muted-foreground/60" />
          </SimpleTooltip>
        </div>
        <p class="text-2xl font-bold">{formatNumber(currentStats.totalAuditsGenerated)}</p>
      </div>
    </div>

    <!-- Tabs: Charts / Jobs / Audit Log -->
    <Tabs.Root bind:value={activeTab}>
      <Tabs.List class="flex border-b border-border">
        <Tabs.Trigger
          value="charts"
          class="px-4 py-2 text-sm font-medium transition-colors
            border-b-2 -mb-px cursor-pointer
            data-[state=active]:border-foreground data-[state=active]:text-foreground
            data-[state=inactive]:border-transparent data-[state=inactive]:text-muted-foreground data-[state=inactive]:hover:text-foreground"
        >
          Charts
        </Tabs.Trigger>
        <Tabs.Trigger
          value="jobs"
          class="px-4 py-2 text-sm font-medium transition-colors
            border-b-2 -mb-px cursor-pointer
            data-[state=active]:border-foreground data-[state=active]:text-foreground
            data-[state=inactive]:border-transparent data-[state=inactive]:text-muted-foreground data-[state=inactive]:hover:text-foreground"
        >
          Jobs
        </Tabs.Trigger>
        <Tabs.Trigger
          value="audit"
          class="px-4 py-2 text-sm font-medium transition-colors
            border-b-2 -mb-px cursor-pointer
            data-[state=active]:border-foreground data-[state=active]:text-foreground
            data-[state=inactive]:border-transparent data-[state=inactive]:text-muted-foreground data-[state=inactive]:hover:text-foreground"
        >
          Audit Log
        </Tabs.Trigger>
      </Tabs.List>

      <Tabs.Content value="charts" class="pt-4">
        <ScraperCharts period={selectedPeriod} term={selectedTerm} />
      </Tabs.Content>

      <Tabs.Content value="jobs" class="pt-4">
        <ScraperJobs active={activeTab === "jobs"} />
      </Tabs.Content>

      <Tabs.Content value="audit" class="pt-4">
        <ScraperAudit active={activeTab === "audit"} />
      </Tabs.Content>
    </Tabs.Root>

    <!-- Subjects Table -->
    <ScraperSubjects subjects={currentSubjects} isLoading={subjects.connectionState !== "connected"} />
  {:else}
    <!-- Initial loading skeleton -->
    <div class="grid grid-cols-2 gap-4 lg:grid-cols-4">
      {#each Array(8) as _, i (i)}
        <div class="bg-card border-border rounded-lg border p-4">
          <div class="h-4 w-24 rounded bg-muted animate-pulse"></div>
          <div class="mt-2 h-8 w-16 rounded bg-muted animate-pulse"></div>
        </div>
      {/each}
    </div>
  {/if}
</div>
