<script lang="ts">
import type { AuditLogEntry } from "$lib/bindings";
import SimpleTooltip from "$lib/components/SimpleTooltip.svelte";
import { FlexRender, createSvelteTable } from "$lib/components/ui/data-table/index.js";
import { useStream } from "$lib/composables";
import { formatAbsoluteDate } from "$lib/date";
import { type DiffEntry, formatDiffPath, jsonDiff, tryParseJson } from "$lib/diff";
import { relativeTime } from "$lib/time";
import { formatNumber } from "$lib/utils";
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

// active prop no longer needed - Tabs.Content handles mount/unmount lifecycle
interface Props {
  active?: boolean;
}
let { active: _active = true }: Props = $props();

let expandedId: number | null = $state(null);

// --- WebSocket stream ---
const stream = useStream("auditLog", null, {
  initial: [] as AuditLogEntry[],
  on: {},
  onSnapshot: (snapshot) => snapshot.entries,
  onDelta: (entries, delta) => {
    const existingIds = new Set(entries.map((e) => e.id));
    const newEntries = delta.entries.filter((e) => !existingIds.has(e.id));
    return [...newEntries, ...entries];
  },
});

const entries = $derived(stream.state);
const connectionState = $derived(stream.connectionState);

// --- Live-updating clock for relative timestamps ---
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

// --- Change column helpers ---

interface ChangeAnalysis {
  kind: "scalar" | "json-single" | "json-multi";
  oldRaw: string;
  newRaw: string;
  diffs: DiffEntry[];
  delta: number | null;
}

function analyzeChange(entry: AuditLogEntry): ChangeAnalysis {
  const parsedOld = tryParseJson(entry.oldValue);
  const parsedNew = tryParseJson(entry.newValue);

  const isJsonOld = typeof parsedOld === "object" && parsedOld !== null;
  const isJsonNew = typeof parsedNew === "object" && parsedNew !== null;

  if (isJsonOld && isJsonNew) {
    const diffs = jsonDiff(parsedOld, parsedNew);
    const kind = diffs.length <= 1 ? "json-single" : "json-multi";
    return { kind, oldRaw: entry.oldValue, newRaw: entry.newValue, diffs, delta: null };
  }

  let delta: number | null = null;
  const numOld = Number(entry.oldValue);
  const numNew = Number(entry.newValue);
  if (
    !Number.isNaN(numOld) &&
    !Number.isNaN(numNew) &&
    entry.oldValue !== "" &&
    entry.newValue !== ""
  ) {
    delta = numNew - numOld;
  }

  return { kind: "scalar", oldRaw: entry.oldValue, newRaw: entry.newValue, diffs: [], delta };
}

function stringify(val: unknown): string {
  if (val === undefined) return "∅";
  if (typeof val === "string") return val;
  return JSON.stringify(val);
}

function toggleExpanded(id: number) {
  expandedId = expandedId === id ? null : id;
}

function formatCourse(entry: AuditLogEntry): string {
  if (entry.subject && entry.courseNumber) {
    return `${entry.subject} ${entry.courseNumber}`;
  }
  return `#${entry.courseId}`;
}

function formatCourseTooltip(entry: AuditLogEntry): string {
  const parts: string[] = [];
  if (entry.courseTitle) parts.push(entry.courseTitle);
  if (entry.crn) parts.push(`CRN ${entry.crn}`);
  parts.push(`ID ${entry.courseId}`);
  return parts.join("\n");
}

// --- TanStack Table ---

let sorting: SortingState = $state([{ id: "time", desc: true }]);

function handleSortingChange(updater: Updater<SortingState>) {
  sorting = typeof updater === "function" ? updater(sorting) : updater;
}

const columns: ColumnDef<AuditLogEntry, unknown>[] = [
  {
    id: "time",
    accessorKey: "timestamp",
    header: "Time",
    enableSorting: true,
  },
  {
    id: "course",
    accessorKey: "courseId",
    header: "Course",
    enableSorting: false,
  },
  {
    id: "field",
    accessorKey: "fieldChanged",
    header: "Field",
    enableSorting: true,
  },
  {
    id: "change",
    accessorFn: () => "",
    header: "Change",
    enableSorting: false,
  },
];

const table = createSvelteTable({
  get data() {
    return entries;
  },
  getRowId: (row) => String(row.id),
  columns,
  state: {
    get sorting() {
      return sorting;
    },
  },
  onSortingChange: handleSortingChange,
  getCoreRowModel: getCoreRowModel(),
  getSortedRowModel: getSortedRowModel<AuditLogEntry>(),
  enableSortingRemoval: true,
});

const skeletonWidths: Record<string, string> = {
  time: "w-24",
  course: "w-20",
  field: "w-20",
  change: "w-40",
};

const columnCount = columns.length;
</script>

<div class="bg-card border-border overflow-hidden rounded-lg border">
  <table class="w-full text-sm">
    <thead>
      {#each table.getHeaderGroups() as headerGroup (headerGroup.id)}
        <tr class="border-b border-border text-left text-muted-foreground">
          {#each headerGroup.headers as header (header.id)}
            <th
              class="px-4 py-3 font-medium"
              class:cursor-pointer={header.column.getCanSort()}
              class:select-none={header.column.getCanSort()}
              onclick={header.column.getToggleSortingHandler()}
            >
              {#if header.column.getCanSort()}
                <span class="inline-flex items-center gap-1">
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
      {#if entries.length === 0 && connectionState !== "connected"}
        <!-- Skeleton loading -->
        {#each Array(20) as _entry, i (i)}
          <tr class="border-b border-border">
            {#each columns as col (col.id)}
              <td class="px-4 py-3">
                <div
                  class="h-4 rounded bg-muted animate-pulse {skeletonWidths[col.id ?? ''] ?? 'w-20'}"
                ></div>
              </td>
            {/each}
          </tr>
        {/each}
      {:else if entries.length === 0}
        <tr>
          <td colspan={columnCount} class="px-4 py-12 text-center text-muted-foreground">
            No audit log entries found.
          </td>
        </tr>
      {:else}
        {#each table.getRowModel().rows as row (row.id)}
          {@const entry = row.original}
          {@const change = analyzeChange(entry)}
          {@const isExpanded = expandedId === entry.id}
          {@const clickable = change.kind === "json-multi"}
          <tr
            class="border-b border-border transition-colors last:border-b-0
              {clickable ? 'cursor-pointer hover:bg-muted/50' : ''}
              {isExpanded ? 'bg-muted/30' : ''}"
            onclick={clickable ? () => toggleExpanded(entry.id) : undefined}
          >
            {#each row.getVisibleCells() as cell (cell.id)}
              {@const colId = cell.column.id}
              {#if colId === "time"}
                {@const rel = relativeTime(new Date(entry.timestamp), now)}
                <td class="px-4 py-3 whitespace-nowrap">
                  <SimpleTooltip text={formatAbsoluteDate(entry.timestamp)} side="right" passthrough>
                    <span class="font-mono text-xs text-muted-foreground">{rel.text === "now" ? "just now" : `${rel.text} ago`}</span>
                  </SimpleTooltip>
                </td>
              {:else if colId === "course"}
                <td class="px-4 py-3 whitespace-nowrap">
                  <SimpleTooltip text={formatCourseTooltip(entry)} side="right" passthrough>
                    <span class="font-mono text-xs text-foreground">{formatCourse(entry)}</span>
                  </SimpleTooltip>
                </td>
              {:else if colId === "field"}
                <td class="px-4 py-3">
                  <span
                    class="inline-block rounded-full bg-muted px-2 py-0.5 font-mono text-xs text-muted-foreground"
                  >
                    {entry.fieldChanged}
                  </span>
                </td>
              {:else if colId === "change"}
                <td class="px-4 py-3">
                  {#if change.kind === "scalar"}
                    <span class="inline-flex items-center gap-1.5 text-sm">
                      {#if change.delta !== null}
                        <span class="text-foreground">{formatNumber(change.delta, { sign: true })}<span class="text-muted-foreground/60">,</span></span>
                      {/if}
                      <span class="text-red-400">{change.oldRaw}</span>
                      <span class="text-muted-foreground/60">→</span>
                      <span class="text-green-600 dark:text-green-400">{change.newRaw}</span>
                    </span>
                  {:else if change.kind === "json-single"}
                    {#if change.diffs.length === 1}
                      {@const d = change.diffs[0]}
                      <span class="font-mono text-xs">
                        <span class="text-muted-foreground">{formatDiffPath(d.path)}:</span> <span class="text-red-400">{stringify(d.oldVal)}</span>
                        <span class="text-muted-foreground"> → </span>
                        <span class="text-green-600 dark:text-green-400">{stringify(d.newVal)}</span>
                      </span>
                    {:else}
                      <span class="text-muted-foreground text-xs italic">No changes</span>
                    {/if}
                  {:else if change.kind === "json-multi"}
                    <span class="inline-flex items-center gap-1.5 text-sm text-muted-foreground">
                      {#if isExpanded}
                        <ChevronDown class="size-3.5 shrink-0" />
                      {:else}
                        <ChevronRight class="size-3.5 shrink-0" />
                      {/if}
                      <span class="underline decoration-dotted underline-offset-2">
                        {formatNumber(change.diffs.length)} fields changed
                      </span>
                    </span>
                  {/if}
                </td>
              {/if}
            {/each}
          </tr>
          <!-- Expandable detail row for multi-path JSON diffs -->
          {#if isExpanded && change.kind === "json-multi"}
            <tr class="border-b border-border last:border-b-0">
              <td colspan={columnCount} class="p-0">
                <div transition:slide={{ duration: 200 }}>
                  <div class="bg-muted/20 px-4 py-3">
                    <div class="flex flex-col gap-y-1.5">
                      {#each change.diffs as d (d.path)}
                        <div class="font-mono text-xs">
                          <span class="text-muted-foreground">{formatDiffPath(d.path)}:</span> <span class="text-red-400">{stringify(d.oldVal)}</span>
                          <span class="text-muted-foreground"> → </span>
                          <span class="text-green-600 dark:text-green-400">{stringify(d.newVal)}</span>
                        </div>
                      {/each}
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
