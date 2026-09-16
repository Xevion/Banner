<script lang="ts">
import { client } from "$lib/api";
import type { ScrapeJobDto, ScrapeJobStatus, ScrapePriority } from "$lib/bindings";
import SortableHeader from "$lib/components/SortableHeader.svelte";
import TableSkeleton from "$lib/components/TableSkeleton.svelte";
import {
  APP_TABLE_FEATURES,
  type AppTableFeatures,
  createSvelteTable,
} from "$lib/components/ui/data-table/index.js";
import { createSortingHandler } from "$lib/composables/sorting";
import { useStream } from "$lib/composables/useStream.svelte";
import { formatAbsoluteDate } from "$lib/date";
import { formatDuration } from "$lib/time";
import { TOOLTIP_SURFACE } from "$lib/tooltipClass";
import { TriangleAlert } from "@lucide/svelte";
import { type ColumnDef, type SortingState } from "@tanstack/table-core";
import { match, P } from "ts-pattern";
import { onMount } from "svelte";
import { SvelteMap } from "svelte/reactivity";

// active prop no longer needed - Tabs.Content handles mount/unmount lifecycle
interface Props {
  active?: boolean;
}
let { active: _active = true }: Props = $props();

let initialized = $state(false);
let error = $state<string | null>(null);
let sorting: SortingState = $state([]);
let tick = $state(0);
let subjectMap = new SvelteMap<string, string>();

// Helper to update a job by ID in an array
function updateById(
  jobs: ScrapeJobDto[],
  id: number,
  updates: Partial<ScrapeJobDto>
): ScrapeJobDto[] {
  return jobs.map((job) => (job.id === id ? { ...job, ...updates } : job));
}

// Use the useStream composable for subscription management
const stream = useStream("scrapeJobs", null, {
  initial: [] as ScrapeJobDto[],
  onSnapshot: (snapshot) => {
    initialized = true;
    return sortJobs(snapshot.jobs);
  },
  on: {
    created: (jobs, e) => sortJobs([...jobs, e.job]),
    locked: (jobs, e) =>
      updateById(jobs, e.id, {
        lockedAt: e.lockedAt,
        status: e.status,
      }),
    completed: (jobs, e) => jobs.filter((j) => j.id !== e.id),
    retried: (jobs, e) =>
      updateById(jobs, e.id, {
        retryCount: e.retryCount,
        queuedAt: e.queuedAt,
        status: e.status,
        lockedAt: null,
      }),
    exhausted: (jobs, e) => updateById(jobs, e.id, { status: "exhausted" }),
    deleted: (jobs, e) => jobs.filter((j) => j.id !== e.id),
  },
});

// Expose jobs as a derived binding for the template
const jobs = $derived(stream.state);
// Shared tooltip state -- single tooltip for all timing cells via event delegation
let tooltipText = $state<string | null>(null);
let tooltipX = $state(0);
let tooltipY = $state(0);

function showTooltip(event: MouseEvent) {
  const target = (event.target as HTMLElement).closest<HTMLElement>("[data-timing-tooltip]");
  if (!target) return;
  tooltipText = target.dataset.timingTooltip ?? null;
  tooltipX = event.clientX;
  tooltipY = event.clientY;
}

function moveTooltip(event: MouseEvent) {
  if (tooltipText === null) return;
  const target = (event.target as HTMLElement).closest<HTMLElement>("[data-timing-tooltip]");
  if (!target) {
    tooltipText = null;
    return;
  }
  tooltipText = target.dataset.timingTooltip ?? null;
  tooltipX = event.clientX;
  tooltipY = event.clientY;
}

function hideTooltip() {
  tooltipText = null;
}

onMount(() => {
  // Tick every second for live time displays
  const tickInterval = setInterval(() => {
    tick++;
  }, 1000);

  // Load subject reference data
  void client.getReference("subject").then((result) => {
    if (result.isOk) {
      subjectMap.clear();
      for (const entry of result.value) {
        subjectMap.set(entry.code, entry.description);
      }
    } else {
      console.warn("Failed to load subject reference data:", result.error.message);
    }
  });

  return () => {
    clearInterval(tickInterval);
  };
});

const handleSortingChange = createSortingHandler(
  () => sorting,
  (next) => {
    sorting = next;
  }
);

const PRIORITY_ORDER: Record<ScrapePriority, number> = {
  critical: 0,
  high: 1,
  medium: 2,
  low: 3,
};

function sortJobs(jobs: ScrapeJobDto[]): ScrapeJobDto[] {
  return [...jobs].sort((a, b) => {
    const pa = PRIORITY_ORDER[a.priority];
    const pb = PRIORITY_ORDER[b.priority];
    if (pa !== pb) return pa - pb;
    return new Date(a.executeAt).getTime() - new Date(b.executeAt).getTime();
  });
}

function getTermCode(job: ScrapeJobDto): string | null {
  return job.targetPayload.term;
}

// The payload union carries no tag of its own, so targetType and the payload shape are
// matched together and a row whose two disagree falls through to the raw JSON.
function formatJobDetails(job: ScrapeJobDto, subjects: Map<string, string>): string {
  return match([job.targetType, job.targetPayload] as const)
    .with(["subject", { subject: P.string }], ([, payload]) => {
      const desc = subjects.get(payload.subject);
      return desc ? `${payload.subject} \u2014 ${desc}` : payload.subject;
    })
    .with(["crnList", { crns: P.array(P.string) }], ([, payload]) => `${payload.crns.length} CRNs`)
    .with(["singleCrn", { crn: P.string }], ([, payload]) => `CRN ${payload.crn}`)
    .with(
      ["courseRange", { subject: P.string, low: P.number, high: P.number }],
      ([, payload]) => `${payload.subject} ${payload.low}\u2013${payload.high}`
    )
    .otherwise(() => JSON.stringify(job.targetPayload));
}

const PRIORITY_COLORS: Record<ScrapePriority, string> = {
  critical: "text-red-500",
  high: "text-orange-500",
  medium: "text-foreground",
  low: "text-muted-foreground",
};

function retryColor(retryCount: number, maxRetries: number): string {
  if (retryCount >= maxRetries && maxRetries > 0) return "text-red-500";
  if (retryCount > 0) return "text-amber-500";
  return "text-muted-foreground";
}

const STATUS_COLORS: Record<ScrapeJobStatus, { text: string; dot: string }> = {
  processing: { text: "text-blue-500", dot: "bg-blue-500" },
  pending: { text: "text-green-500", dot: "bg-green-500" },
  scheduled: { text: "text-muted-foreground", dot: "bg-muted-foreground" },
  staleLock: { text: "text-red-500", dot: "bg-red-500" },
  exhausted: { text: "text-red-500", dot: "bg-red-500" },
};

function formatStatusLabel(status: string): string {
  // Convert camelCase to separate words, capitalize first letter
  return status.replace(/([a-z])([A-Z])/g, "$1 $2").replace(/^\w/, (c) => c.toUpperCase());
}

function lockDurationColor(ms: number): string {
  const minutes = ms / 60_000;
  if (minutes >= 8) return "text-red-500";
  if (minutes >= 5) return "text-amber-500";
  return "text-foreground";
}

function overdueDurationColor(ms: number): string {
  const minutes = ms / 60_000;
  if (minutes >= 5) return "text-red-500";
  return "text-amber-500";
}

const columns: ColumnDef<AppTableFeatures, ScrapeJobDto>[] = [
  {
    id: "id",
    accessorKey: "id",
    header: "ID",
    enableSorting: false,
  },
  {
    id: "status",
    accessorKey: "status",
    header: "Status",
    enableSorting: true,
    sortFn: (rowA, rowB) => {
      const order: Record<ScrapeJobStatus, number> = {
        processing: 0,
        staleLock: 1,
        pending: 2,
        scheduled: 3,
        exhausted: 4,
      };
      return order[rowA.original.status] - order[rowB.original.status];
    },
  },
  {
    id: "targetType",
    accessorKey: "targetType",
    header: "Type",
    enableSorting: false,
  },
  {
    id: "details",
    accessorFn: () => "",
    header: "Details",
    enableSorting: false,
  },
  {
    id: "term",
    accessorFn: (row) => getTermCode(row) ?? "",
    header: "Term",
    enableSorting: true,
  },
  {
    id: "priority",
    accessorKey: "priority",
    header: "Priority",
    enableSorting: true,
    sortFn: (rowA, rowB) =>
      PRIORITY_ORDER[rowA.original.priority] - PRIORITY_ORDER[rowB.original.priority],
  },
  {
    id: "timing",
    accessorFn: (row) => {
      if (row.lockedAt) return Date.now() - new Date(row.lockedAt).getTime();
      return Date.now() - new Date(row.queuedAt).getTime();
    },
    header: "Timing",
    enableSorting: true,
  },
];

const table = createSvelteTable({
  features: APP_TABLE_FEATURES,
  get data() {
    return jobs;
  },
  getRowId: (row) => String(row.id),
  columns,
  state: {
    get sorting() {
      return sorting;
    },
  },
  onSortingChange: handleSortingChange,
  enableSortingRemoval: true,
});

const skeletonWidths: Record<string, string> = {
  id: "w-6",
  status: "w-20",
  targetType: "w-16",
  details: "w-32",
  term: "w-16",
  priority: "w-16",
  timing: "w-32",
};

// Unified timing display: shows the most relevant duration for the job's current state.
// Uses tick dependency so Svelte re-evaluates every second.
function getTimingDisplay(
  job: ScrapeJobDto,
  _tick: number
): {
  text: string;
  colorClass: string;
  icon: "warning" | "none";
  tooltip: string;
} {
  const now = Date.now();
  const queuedTime = new Date(job.queuedAt).getTime();
  const executeTime = new Date(job.executeAt).getTime();

  const executeAtDiff = now - executeTime;

  // A job overdue for execution reads as scheduled whatever its own status says.
  const scheduled = () => {
    const tooltipLines = [
      `Queued: ${formatAbsoluteDate(job.queuedAt)}`,
      `Executes: ${formatAbsoluteDate(job.executeAt)}`,
    ];
    return {
      text: `in ${formatDuration(Math.abs(executeAtDiff))}`,
      colorClass: "text-muted-foreground",
      icon: "none" as const,
      tooltip: tooltipLines.join("\n"),
    };
  };

  const processing = () => {
    const lockedTime = job.lockedAt ? new Date(job.lockedAt).getTime() : now;
    const processingMs = now - lockedTime;
    const waitedMs = lockedTime - queuedTime;

    const prefix = job.status === "staleLock" ? "stale" : "processing";
    const colorClass =
      job.status === "staleLock" ? "text-red-500" : lockDurationColor(processingMs);

    const tooltipLines = [
      `Queued: ${formatAbsoluteDate(job.queuedAt)}`,
      `Waited: ${formatDuration(Math.max(0, waitedMs))}`,
    ];
    if (job.lockedAt) {
      tooltipLines.push(`Locked: ${formatAbsoluteDate(job.lockedAt)}`);
    }
    tooltipLines.push(
      `${job.status === "staleLock" ? "Stale for" : "Processing"}: ${formatDuration(processingMs)}`
    );

    return {
      text: `${prefix} ${formatDuration(processingMs)}`,
      colorClass,
      icon: job.status === "staleLock" ? ("warning" as const) : ("none" as const),
      tooltip: tooltipLines.join("\n"),
    };
  };

  const exhausted = () => ({
    text: "exhausted",
    colorClass: "text-red-500",
    icon: "warning" as const,
    tooltip: [
      `Queued: ${formatAbsoluteDate(job.queuedAt)}`,
      `Retries: ${job.retryCount}/${job.maxRetries} exhausted`,
    ].join("\n"),
  });

  // Overdue: execute_at is in the past and nothing has picked the job up.
  const waiting = () => {
    const waitingMs = now - queuedTime;
    return {
      text: `waiting ${formatDuration(waitingMs)}`,
      colorClass: overdueDurationColor(waitingMs),
      icon: "warning" as const,
      tooltip: [
        `Queued: ${formatAbsoluteDate(job.queuedAt)}`,
        `Waiting: ${formatDuration(waitingMs)}`,
      ].join("\n"),
    };
  };

  return match(job.status)
    .with("processing", "staleLock", processing)
    .with("exhausted", exhausted)
    .with("scheduled", scheduled)
    .with("pending", () => (executeAtDiff < 0 ? scheduled() : waiting()))
    .exhaustive();
}
</script>

{#snippet idCell(job: ScrapeJobDto)}
  <td class="px-3 py-2.5 tabular-nums text-muted-foreground/70 w-12">{job.id}</td>
{/snippet}

{#snippet statusCell(job: ScrapeJobDto)}
  {@const sc = STATUS_COLORS[job.status]}
  <td class="px-3 py-2.5 whitespace-nowrap">
    <span class="inline-flex items-center gap-1.5">
      <span class="size-1.5 shrink-0 rounded-full {sc.dot}"></span>
      <span class="flex flex-col leading-tight">
        <span class={sc.text}>{formatStatusLabel(job.status)}</span>
        {#if job.maxRetries > 0}
          <span class="text-[10px] {retryColor(job.retryCount, job.maxRetries)}">
            {job.retryCount}/{job.maxRetries} retries
          </span>
        {/if}
      </span>
    </span>
  </td>
{/snippet}

{#snippet targetTypeCell(job: ScrapeJobDto)}
  <td class="px-3 py-2.5 whitespace-nowrap">
    <span
      class="inline-flex items-center rounded-md bg-muted/60 px-1.5 py-0.5 font-mono text-[11px] text-muted-foreground"
    >
      {formatStatusLabel(job.targetType)}
    </span>
  </td>
{/snippet}

{#snippet detailsCell(job: ScrapeJobDto)}
  <td
    class="px-3 py-2.5 max-w-48 truncate text-muted-foreground"
    title={formatJobDetails(job, subjectMap)}
  >
    {formatJobDetails(job, subjectMap)}
  </td>
{/snippet}

{#snippet termCell(job: ScrapeJobDto)}
  <td class="px-3 py-2.5 whitespace-nowrap">
    {#if getTermCode(job)}
      <span class="font-mono text-xs text-muted-foreground">{getTermCode(job)}</span>
    {:else}
      <span class="text-xs text-muted-foreground/40">&mdash;</span>
    {/if}
  </td>
{/snippet}

{#snippet priorityCell(job: ScrapeJobDto)}
  <td class="px-3 py-2.5 whitespace-nowrap">
    <span class="font-medium capitalize {PRIORITY_COLORS[job.priority]}">
      {job.priority}
    </span>
  </td>
{/snippet}

{#snippet timingCell(job: ScrapeJobDto)}
  {@const timingDisplay = getTimingDisplay(job, tick)}
  <td class="px-3 py-2.5 whitespace-nowrap">
    <span
      class="inline-flex items-center gap-1.5 tabular-nums text-foreground"
      data-timing-tooltip={timingDisplay.tooltip}
    >
      <span
        class="size-3.5 shrink-0 inline-flex items-center justify-center {timingDisplay.colorClass}"
      >
        {#if timingDisplay.icon === "warning"}
          <TriangleAlert class="size-3.5" />
        {/if}
      </span>
      {timingDisplay.text}
    </span>
  </td>
{/snippet}

{#snippet jobCell(colId: string, job: ScrapeJobDto)}
  {#if colId === "id"}
    {@render idCell(job)}
  {:else if colId === "status"}
    {@render statusCell(job)}
  {:else if colId === "targetType"}
    {@render targetTypeCell(job)}
  {:else if colId === "details"}
    {@render detailsCell(job)}
  {:else if colId === "term"}
    {@render termCell(job)}
  {:else if colId === "priority"}
    {@render priorityCell(job)}
  {:else if colId === "timing"}
    {@render timingCell(job)}
  {/if}
{/snippet}

{#if error}
  <p class="text-destructive">{error}</p>
{:else}
  <div class="bg-card border-border overflow-hidden rounded-lg border">
    <table
      class="w-full border-collapse text-xs"
      onmouseenter={showTooltip}
      onmousemove={moveTooltip}
      onmouseleave={hideTooltip}
    >
      <SortableHeader headerGroups={table.getHeaderGroups()} />
      {#if !initialized}
        <TableSkeleton {columns} rowCount={5} {skeletonWidths} />
      {:else if jobs.length === 0}
        <tbody>
          <tr>
            <td
              colspan={columns.length}
              class="py-12 text-center text-muted-foreground"
            >
              No scrape jobs found.
            </td>
          </tr>
        </tbody>
      {:else}
        <tbody>
          {#each table.getRowModel().rows as row (row.id)}
            {@const job = row.original}
            <tr
              class="border-b border-border last:border-b-0 hover:bg-muted/50 transition-colors"
            >
              {#each row.getVisibleCells() as cell (cell.id)}
                {@render jobCell(cell.column.id, job)}
              {/each}
            </tr>
          {/each}
        </tbody>
      {/if}
    </table>
  </div>

  {#if tooltipText !== null}
    <div
      class="pointer-events-none fixed {TOOLTIP_SURFACE}"
      style="left: {tooltipX + 12}px; top: {tooltipY + 12}px;"
    >
      {tooltipText}
    </div>
  {/if}
{/if}
