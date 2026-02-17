<script lang="ts">
import type { SubjectDetailResponse, SubjectSummary } from "$lib/bindings";
import { client } from "$lib/api";
import SimpleTooltip from "$lib/components/SimpleTooltip.svelte";
import { FlexRender, createSvelteTable } from "$lib/components/ui/data-table/index.js";
import { formatAbsoluteDate } from "$lib/date";
import { formatDuration, formatDurationMs, relativeTime } from "$lib/time";
import { ArrowDown, ArrowUp, ArrowUpDown, ChevronDown, ChevronRight } from "@lucide/svelte";
import {
  type ColumnDef,
  type SortingState,
  type Updater,
  getCoreRowModel,
  getSortedRowModel,
} from "@tanstack/table-core";
import { onDestroy } from "svelte";
import { slide } from "svelte/transition";

interface Props {
  subjects: SubjectSummary[];
  isLoading?: boolean;
}

let { subjects, isLoading = false }: Props = $props();

let expandedSubject = $state<string | null>(null);
let subjectDetail = $state<SubjectDetailResponse | null>(null);
let detailLoading = $state(false);

async function toggleSubjectDetail(subject: string) {
  if (expandedSubject === subject) {
    expandedSubject = null;
    subjectDetail = null;
    return;
  }
  expandedSubject = subject;
  detailLoading = true;
  const result = await client.getScraperSubjectDetail(subject);
  subjectDetail = result.isOk ? result.value : null;
  detailLoading = false;
}

let now = $state(new Date());
let tickTimer: ReturnType<typeof setTimeout> | undefined;

function scheduleTick() {
  tickTimer = setTimeout(() => {
    now = new Date();
    scheduleTick();
  }, 1000);
}

scheduleTick();

onDestroy(() => {
  clearTimeout(tickTimer);
});

function formatInterval(secs: number): string {
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.round(secs / 60)}m`;
  return `${(secs / 3600).toFixed(1)}h`;
}

function emphasisClass(value: number): string {
  return value === 0 ? "text-muted-foreground" : "text-foreground";
}

let sorting: SortingState = $state([{ id: "subject", desc: false }]);

function handleSortingChange(updater: Updater<SortingState>) {
  sorting = typeof updater === "function" ? updater(sorting) : updater;
}

const columns: ColumnDef<SubjectSummary, unknown>[] = [
  {
    id: "subject",
    accessorKey: "subject",
    header: "Subject",
    enableSorting: true,
    sortingFn: (a, b) => a.original.subject.localeCompare(b.original.subject),
  },
  {
    id: "status",
    accessorFn: (row) => row.scheduleState,
    header: "Scrape in",
    enableSorting: true,
    sortingFn: (a, b) => {
      const order: Record<string, number> = { eligible: 0, cooldown: 1, paused: 2, read_only: 3 };
      const sa = order[a.original.scheduleState] ?? 4;
      const sb = order[b.original.scheduleState] ?? 4;
      if (sa !== sb) return sa - sb;
      return (
        (a.original.cooldownRemainingSecs ?? Infinity) -
        (b.original.cooldownRemainingSecs ?? Infinity)
      );
    },
  },
  {
    id: "interval",
    accessorFn: (row) => row.currentIntervalSecs * row.timeMultiplier,
    header: "Interval",
    enableSorting: true,
  },
  {
    id: "lastScraped",
    accessorKey: "lastScraped",
    header: "Last Scraped",
    enableSorting: true,
  },
  {
    id: "changeRate",
    accessorKey: "avgChangeRatio",
    header: "Change %",
    enableSorting: true,
  },
  {
    id: "zeros",
    accessorKey: "consecutiveZeroChanges",
    header: "Zeros",
    enableSorting: true,
  },
  {
    id: "runs",
    accessorKey: "recentRuns",
    header: "Runs",
    enableSorting: true,
  },
  {
    id: "fails",
    accessorKey: "recentFailures",
    header: "Fails",
    enableSorting: true,
  },
];

const table = createSvelteTable({
  get data() {
    return subjects;
  },
  getRowId: (row) => row.subject,
  columns,
  state: {
    get sorting() {
      return sorting;
    },
  },
  onSortingChange: handleSortingChange,
  getCoreRowModel: getCoreRowModel(),
  getSortedRowModel: getSortedRowModel<SubjectSummary>(),
  enableSortingRemoval: true,
});

const skeletonWidths: Record<string, string> = {
  subject: "w-24",
  status: "w-20",
  interval: "w-14",
  lastScraped: "w-20",
  changeRate: "w-12",
  zeros: "w-8",
  runs: "w-8",
  fails: "w-8",
};

const columnCount = columns.length;
const detailGridCols = "grid-cols-[7fr_5fr_3fr_4fr_4fr_3fr_4fr_minmax(6rem,1fr)]";
</script>

<div class="bg-card border-border rounded-lg border">
  <h2 class="border-border border-b px-3 py-2.5 text-xs font-semibold text-foreground">
    Subjects ({subjects.length})
  </h2>
  <div class="overflow-x-auto">
    <table class="w-full min-w-160 border-collapse text-xs">
      <thead>
        {#each table.getHeaderGroups() as headerGroup (headerGroup.id)}
          <tr class="border-border border-b text-left text-muted-foreground">
            {#each headerGroup.headers as header (header.id)}
              <th
                class="px-3 py-1.5 text-[10px] font-medium uppercase tracking-wider whitespace-nowrap"
                class:cursor-pointer={header.column.getCanSort()}
                class:select-none={header.column.getCanSort()}
                onclick={header.column.getToggleSortingHandler()}
              >
                {#if header.column.getCanSort()}
                  <span class="inline-flex items-center gap-1 hover:text-foreground">
                    {#if typeof header.column.columnDef.header === "string"}
                      {header.column.columnDef.header}
                    {:else}
                      <FlexRender
                        content={header.column.columnDef.header}
                        context={header.getContext()}
                      />
                    {/if}
                    {#if header.column.getIsSorted() === "asc"}
                      <ArrowUp class="size-3.5" />
                    {:else if header.column.getIsSorted() === "desc"}
                      <ArrowDown class="size-3.5" />
                    {:else}
                      <ArrowUpDown class="size-3.5 text-muted-foreground/40" />
                    {/if}
                  </span>
                {:else if typeof header.column.columnDef.header === "string"}
                  {header.column.columnDef.header}
                {:else}
                  <FlexRender
                    content={header.column.columnDef.header}
                    context={header.getContext()}
                  />
                {/if}
              </th>
            {/each}
          </tr>
        {/each}
      </thead>
      <tbody>
        {#if isLoading && subjects.length === 0}
          <!-- Skeleton loading -->
          {#each Array(12) as _, i (i)}
            <tr class="border-border border-b">
              {#each columns as col (col.id)}
                <td class="px-3 py-2">
                  <div
                    class="h-3.5 rounded bg-muted animate-pulse {skeletonWidths[col.id ?? ''] ?? 'w-16'}"
                  ></div>
                </td>
              {/each}
            </tr>
          {/each}
        {:else if subjects.length === 0}
          <tr>
            <td colspan={columnCount} class="px-3 py-8 text-center text-muted-foreground">
              No subjects found.
            </td>
          </tr>
        {:else}
          {#each table.getRowModel().rows as row (row.id)}
            {@const subject = row.original}
            {@const isExpanded = expandedSubject === subject.subject}
            {@const rel = relativeTime(new Date(subject.lastScraped), now)}
            <tr
              class="border-border cursor-pointer border-b transition-colors hover:bg-muted/50
                {isExpanded ? 'bg-muted/30' : ''}"
              onclick={() => toggleSubjectDetail(subject.subject)}
            >
              {#each row.getVisibleCells() as cell (cell.id)}
                {@const colId = cell.column.id}
                {#if colId === "subject"}
                  <td class="px-3 py-1.5 font-medium">
                    <div class="flex items-center gap-1.5">
                      {#if isExpanded}
                        <ChevronDown size={12} class="shrink-0" />
                      {:else}
                        <ChevronRight size={12} class="shrink-0" />
                      {/if}
                      <span>{subject.subject}</span>
                      {#if subject.subjectDescription}
                        <span
                          class="text-muted-foreground font-normal text-[10px] max-w-[140px] truncate inline-block align-middle"
                          title={subject.subjectDescription}
                        >{subject.subjectDescription}</span>
                      {/if}
                      {#if subject.trackedCourseCount > 0}
                        <span class="text-muted-foreground/60 font-normal text-[10px]">({subject.trackedCourseCount})</span>
                      {/if}
                    </div>
                  </td>
                {:else if colId === "status"}
                  <td class="px-3 py-1.5">
                    {#if subject.scheduleState === "paused"}
                      <span class="text-orange-600 dark:text-orange-400">paused</span>
                    {:else if subject.scheduleState === "read_only"}
                      <span class="text-muted-foreground">read only</span>
                    {:else if subject.nextEligibleAt}
                      {@const remainingMs = new Date(subject.nextEligibleAt).getTime() - now.getTime()}
                      {#if remainingMs >= 1000}
                        <span class="text-muted-foreground">{formatDuration(remainingMs)}</span>
                      {:else}
                        <span class="text-green-600 dark:text-green-400 font-medium">ready</span>
                      {/if}
                    {:else}
                      <span class="text-green-600 dark:text-green-400 font-medium">ready</span>
                    {/if}
                  </td>
                {:else if colId === "interval"}
                  <td class="px-3 py-1.5">
                    <span>{formatInterval(subject.currentIntervalSecs)}</span>
                    {#if subject.timeMultiplier !== 1}
                      <span class="text-muted-foreground ml-0.5">&times;{subject.timeMultiplier}</span>
                    {/if}
                  </td>
                {:else if colId === "lastScraped"}
                  <td class="px-3 py-1.5">
                    <SimpleTooltip text={formatAbsoluteDate(subject.lastScraped)} side="top" passthrough>
                      <span class="text-muted-foreground">{rel.text === "now" ? "just now" : rel.text}</span>
                    </SimpleTooltip>
                  </td>
                {:else if colId === "changeRate"}
                  <td class="px-3 py-1.5">
                    <span class={emphasisClass(subject.avgChangeRatio)}>{(subject.avgChangeRatio * 100).toFixed(2)}%</span>
                  </td>
                {:else if colId === "zeros"}
                  <td class="px-3 py-1.5">
                    <span class={emphasisClass(subject.consecutiveZeroChanges)}>{subject.consecutiveZeroChanges}</span>
                  </td>
                {:else if colId === "runs"}
                  <td class="px-3 py-1.5">
                    <span class={emphasisClass(subject.recentRuns)}>{subject.recentRuns}</span>
                  </td>
                {:else if colId === "fails"}
                  <td class="px-3 py-1.5">
                    {#if subject.recentFailures > 0}
                      <span class="text-red-600 dark:text-red-400">{subject.recentFailures}</span>
                    {:else}
                      <span class="text-muted-foreground">{subject.recentFailures}</span>
                    {/if}
                  </td>
                {/if}
              {/each}
            </tr>
            <!-- Expanded Detail -->
            {#if isExpanded}
              <tr class="border-border border-b last:border-b-0">
                <td colspan={columnCount} class="p-0">
                  <div transition:slide={{ duration: 200 }}>
                    <div class="bg-muted/40 px-4 py-3">
                        <div class="text-xs overflow-x-auto">
                          <div class="min-w-fit">
                          <!-- Header (outside scroll region) -->
                          <div class="grid {detailGridCols} text-muted-foreground border-border/50 border-b">
                            <div class="px-3 py-1.5 font-medium">Time</div>
                            <div class="px-3 py-1.5 font-medium">Duration</div>
                            <div class="px-3 py-1.5 font-medium">Status</div>
                            <div class="px-3 py-1.5 font-medium">Fetched</div>
                            <div class="px-3 py-1.5 font-medium">Changed</div>
                            <div class="px-3 py-1.5 font-medium">%</div>
                            <div class="px-3 py-1.5 font-medium">Audits</div>
                            <div class="px-3 py-1.5 font-medium">Error</div>
                          </div>
                          <!-- Body (scrollable vertically, horizontal clipped to match header) -->
                          <div class="max-h-[280px] overflow-y-auto overflow-x-hidden">
                            {#if detailLoading}
                              {#each Array(8) as _, i (i)}
                                <div class="grid {detailGridCols} border-border/50 border-t">
                                  <div class="px-3 py-1.5"><div class="h-3.5 w-16 rounded bg-muted animate-pulse"></div></div>
                                  <div class="px-3 py-1.5"><div class="h-3.5 w-12 rounded bg-muted animate-pulse"></div></div>
                                  <div class="px-3 py-1.5"><div class="h-3.5 w-8 rounded bg-muted animate-pulse"></div></div>
                                  <div class="px-3 py-1.5"><div class="h-3.5 w-10 rounded bg-muted animate-pulse"></div></div>
                                  <div class="px-3 py-1.5"><div class="h-3.5 w-10 rounded bg-muted animate-pulse"></div></div>
                                  <div class="px-3 py-1.5"><div class="h-3.5 w-8 rounded bg-muted animate-pulse"></div></div>
                                  <div class="px-3 py-1.5"><div class="h-3.5 w-8 rounded bg-muted animate-pulse"></div></div>
                                  <div class="px-3 py-1.5"><div class="h-3.5 w-16 rounded bg-muted animate-pulse"></div></div>
                                </div>
                              {/each}
                            {:else if subjectDetail && subjectDetail.results.length > 0}
                              {#each subjectDetail.results as result (result.id)}
                                {@const detailRel = relativeTime(new Date(result.completedAt), now)}
                                <div class="grid {detailGridCols} border-border/50 border-t">
                                  <div class="px-3 py-1.5">
                                    <SimpleTooltip text={formatAbsoluteDate(result.completedAt)} side="top" passthrough>
                                      <span class="inline-block min-w-[4.5rem] text-muted-foreground">{detailRel.text === "now" ? "just now" : detailRel.text}</span>
                                    </SimpleTooltip>
                                  </div>
                                  <div class="px-3 py-1.5">{formatDurationMs(result.durationMs)}</div>
                                  <div class="px-3 py-1.5">
                                    {#if result.success}
                                      <span class="text-green-600 dark:text-green-400">ok</span>
                                    {:else}
                                      <span class="text-red-600 dark:text-red-400">fail</span>
                                    {/if}
                                  </div>
                                  <div class="px-3 py-1.5">
                                    <span class={emphasisClass(result.coursesFetched ?? 0)}>{result.coursesFetched ?? "\u2014"}</span>
                                  </div>
                                  <div class="px-3 py-1.5">
                                    <span class={emphasisClass(result.coursesChanged ?? 0)}>{result.coursesChanged ?? "\u2014"}</span>
                                  </div>
                                  <div class="px-3 py-1.5">
                                    {#if result.coursesFetched != null && result.coursesFetched > 0 && result.coursesChanged != null}
                                      <span class={emphasisClass(result.coursesChanged)}>{(result.coursesChanged / result.coursesFetched * 100).toFixed(1)}%</span>
                                    {:else}
                                      <span class="text-muted-foreground">—</span>
                                    {/if}
                                  </div>
                                  <div class="px-3 py-1.5">
                                    <span class={emphasisClass(result.auditsGenerated ?? 0)}>{result.auditsGenerated ?? "\u2014"}</span>
                                  </div>
                                  <div class="px-3 py-1.5">
                                    {#if !result.success && result.errorMessage}
                                      <SimpleTooltip text={result.errorMessage} side="top" passthrough>
                                        <span class="text-red-600 dark:text-red-400 max-w-[12rem] truncate inline-block align-middle">{result.errorMessage}</span>
                                      </SimpleTooltip>
                                    {:else}
                                      <span class="text-muted-foreground">—</span>
                                    {/if}
                                  </div>
                                </div>
                              {/each}
                            {:else}
                              <div class="px-3 py-4 text-center text-muted-foreground text-sm">No recent results.</div>
                            {/if}
                          </div>
                          </div>
                        </div>
                    </div>
                  </div>
                </td>
              </tr>
            {/if}
          {/each}
        {/if}
      </tbody>
    </table>
  </div>
</div>
