<script lang="ts">
import type { AuditLogEntry } from "$lib/bindings";
import SimpleTooltip from "$lib/components/SimpleTooltip.svelte";
import SortableHeader from "$lib/components/SortableHeader.svelte";
import TableSkeleton from "$lib/components/TableSkeleton.svelte";
import { createSvelteTable } from "$lib/components/ui/data-table/index.js";
import { createSortingHandler } from "$lib/composables/sorting";
import { useStream } from "$lib/composables/useStream.svelte";
import { formatAbsoluteDate } from "$lib/date";
import { type DiffEntry, formatDiffPath, jsonDiff } from "$lib/diff";
import { relativeTime } from "$lib/time";
import { formatNumber } from "$lib/utils";
import { ChevronDown, ChevronRight } from "@lucide/svelte";
import {
  type ColumnDef,
  type SortingState,
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

interface ChangeAnalysis {
  kind: "scalar" | "json-single" | "json-multi";
  oldRaw: string;
  newRaw: string;
  diffs: DiffEntry[];
  delta: number | null;
}

function displayValue(val: unknown): string {
  if (val === null || val === undefined) return "";
  if (typeof val === "string") return val;
  return JSON.stringify(val);
}

function analyzeChange(entry: AuditLogEntry): ChangeAnalysis {
  const oldVal = entry.oldValue;
  const newVal = entry.newValue;

  const isJsonOld = typeof oldVal === "object" && oldVal !== null;
  const isJsonNew = typeof newVal === "object" && newVal !== null;

  if (isJsonOld && isJsonNew) {
    const diffs = jsonDiff(oldVal, newVal);
    const kind = diffs.length <= 1 ? "json-single" : "json-multi";
    return { kind, oldRaw: displayValue(oldVal), newRaw: displayValue(newVal), diffs, delta: null };
  }

  let delta: number | null = null;
  if (typeof oldVal === "number" && typeof newVal === "number") {
    delta = newVal - oldVal;
  }

  return {
    kind: "scalar",
    oldRaw: displayValue(oldVal),
    newRaw: displayValue(newVal),
    diffs: [],
    delta,
  };
}

function stringify(val: unknown): string {
  if (val === undefined) return "(none)";
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

let sorting: SortingState = $state([{ id: "time", desc: true }]);

const handleSortingChange = createSortingHandler(
  () => sorting,
  (next) => {
    sorting = next;
  }
);

const columns: ColumnDef<AuditLogEntry, unknown>[] = [
  {
    id: "time",
    accessorKey: "timestamp",
    header: "Time",
    enableSorting: true,
  },
  {
    id: "term",
    accessorKey: "termCode",
    header: "Term",
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
  term: "w-16",
  course: "w-20",
  field: "w-20",
  change: "w-40",
};

const columnCount = columns.length;
</script>

<div class="bg-card border-border overflow-hidden rounded-lg border">
  <table class="w-full text-sm">
    <SortableHeader headerGroups={table.getHeaderGroups()} thClass="px-4 py-3 font-medium" />
    {#if entries.length === 0 && connectionState !== "connected"}
      <TableSkeleton {columns} rowCount={20} {skeletonWidths} cellClass="px-4 py-3" rowHeight="h-4" />
    {:else}
    <tbody>
      {#if entries.length === 0}
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
              {:else if colId === "term"}
                <td class="px-4 py-3 whitespace-nowrap">
                  {#if entry.termCode}
                    <span class="font-mono text-xs text-muted-foreground">{entry.termCode}</span>
                  {:else}
                    <span class="text-xs text-muted-foreground/40">&mdash;</span>
                  {/if}
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
                      <span class="text-muted-foreground/60">&rarr;</span>
                      <span class="text-green-600 dark:text-green-400">{change.newRaw}</span>
                    </span>
                  {:else if change.kind === "json-single"}
                    {#if change.diffs.length === 1}
                      {@const d = change.diffs[0]}
                      <span class="font-mono text-xs">
                        <span class="text-muted-foreground">{formatDiffPath(d.path)}:</span> <span class="text-red-400">{stringify(d.oldVal)}</span>
                        <span class="text-muted-foreground"> &rarr; </span>
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
                          <span class="text-muted-foreground"> &rarr; </span>
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
    {/if}
  </table>
</div>
