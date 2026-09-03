<script lang="ts">
import { Check, RotateCcw } from "@lucide/svelte";
import { getCoreRowModel, type Updater, type VisibilityState } from "@tanstack/table-core";
import { ContextMenu } from "bits-ui";
import { flip } from "svelte/animate";
import { fade, slide } from "svelte/transition";
import type { CourseResponse, SortKeyOption } from "$lib/bindings";
import CourseDetail from "$lib/components/CourseDetail.svelte";
import SortableHeader, { type HeaderOverride } from "$lib/components/SortableHeader.svelte";
import { createSvelteTable } from "$lib/components/ui/data-table/index.js";
import { useClipboard } from "$lib/composables/useClipboard.svelte";
import { useOverlayScrollbars } from "$lib/composables/useOverlayScrollbars.svelte";
import { useTooltipDelegation } from "$lib/composables/useTooltipDelegation";
import { timeSpansPair } from "$lib/scheduleState";
import { applyHeaderSort, headerSortStep, type SortTerm } from "$lib/sort";
import { COLUMN_DEFS, COLUMNS, FLEX_COLUMN, tableMinWidth } from "./columns";
import { setTableContext } from "./context";
import EmptyState from "./EmptyState.svelte";
import { buildSkeletonHtml } from "./skeletons";

let {
  courses,
  loading,
  stale,
  sorting = [],
  sortOptions = [],
  onSortingChange,
  subjectMap = {},
  columnVisibility = $bindable({}),
  defaultVisibility = {},
  expandedCrn,
  onToggle,
  skeletonRowCount,
  hadResults,
  observeHeight,
  contentHeight,
}: {
  courses: CourseResponse[];
  loading: boolean;
  stale: boolean;
  sorting?: SortTerm[];
  sortOptions?: SortKeyOption[];
  onSortingChange?: (sorting: SortTerm[]) => void;
  subjectMap?: Record<string, string>;
  columnVisibility?: VisibilityState;
  defaultVisibility?: VisibilityState;
  expandedCrn: string | null;
  onToggle: (crn: string) => void;
  skeletonRowCount: number;
  hadResults: boolean;
  observeHeight: (el: HTMLTableElement) => () => void;
  contentHeight: number | null;
} = $props();

let tableWrapper: HTMLDivElement = undefined!;
let tableElement: HTMLTableElement = undefined!;
const clipboard = useClipboard(1000);

// Set context once for all cells - shared utilities
setTableContext({
  clipboard,
  get subjectMap() {
    return subjectMap;
  },
  get maxSubjectLength() {
    return maxSubjectLength;
  },
  isColumnVisible: (id: string) => columnVisibility[id] !== false,
});

useOverlayScrollbars(() => tableWrapper, {
  overflow: { x: "scroll", y: "hidden" },
  scrollbars: { autoHide: "never" },
});

// Singleton tooltip delegation
$effect(() => {
  if (!tableElement) return;
  const tooltipDelegation = useTooltipDelegation(tableElement);
  return () => tooltipDelegation.destroy();
});

// Height observation via composable
$effect(() => {
  if (!tableElement) return;
  return observeHeight(tableElement);
});

let maxSubjectLength = $derived(
  courses.length > 0 ? Math.max(...courses.map((c) => c.subject.length)) : 3
);

let visibleColumnIds = $derived(
  COLUMN_DEFS.map((c) => c.id).filter((id) => columnVisibility[id] !== false)
);

// Measured against the default, not against "nothing hidden": a column that
// starts hidden must not make the table look permanently customised.
let hiddenColumnIds = $derived(
  Object.entries(columnVisibility)
    .filter(([, visible]) => visible === false)
    .map(([id]) => id)
);
let defaultHiddenIds = $derived(
  Object.entries(defaultVisibility)
    .filter(([, visible]) => visible === false)
    .map(([id]) => id)
);
let hasCustomVisibility = $derived(
  hiddenColumnIds.length !== defaultHiddenIds.length ||
    defaultHiddenIds.some((id) => !hiddenColumnIds.includes(id))
);

function resetColumnVisibility() {
  columnVisibility = { ...defaultVisibility };
}

function handleVisibilityChange(updater: Updater<VisibilityState>) {
  const newVisibility = typeof updater === "function" ? updater(columnVisibility) : updater;
  columnVisibility = newVisibility;
}

/**
 * Every sortable header runs the same cycle: each key the column offers, both
 * ways round, then off. The instructor header's five states are that rule on a
 * two-key column rather than a mechanism of its own.
 */
const sortLabels = $derived(new Map(sortOptions.map((option) => [option.key, option])));

function courseHeaderOverride(headerId: string): HeaderOverride | null {
  const step = headerSortStep(headerId, sorting, sortLabels);
  if (!step) return null;

  // Shown together the columns are one range under one label, so the titles are
  // left to say which half each sort control orders.
  const paired = columnVisibility.time !== false && columnVisibility.time_end !== false;
  let label: string | undefined;
  if (paired && headerId === "time") label = "Time";
  if (paired && headerId === "time_end") label = "";

  return {
    label,
    suffix: step.suffix,
    indicator: step.indicator,
    title: step.title,
    onclick: () => onSortingChange?.(applyHeaderSort(step.next)),
  };
}

const table = createSvelteTable({
  get data() {
    return courses;
  },
  getRowId: (row) => String(row.crn),
  columns: COLUMN_DEFS,
  state: {
    get columnVisibility() {
      return columnVisibility;
    },
  },
  onColumnVisibilityChange: handleVisibilityChange,
  getCoreRowModel: getCoreRowModel(),
});
</script>

<!-- Desktop table
     IMPORTANT: !important flags on hidden/block are required because OverlayScrollbars
     applies inline styles (style="display: ...") to set up its custom scrollbar UI. -->
<div
  bind:this={tableWrapper}
  class="!hidden sm:!block overflow-x-auto overflow-y-hidden transition-[height] duration-200"
  style:height={contentHeight != null ? `${contentHeight}px` : undefined}
  style:view-transition-name="search-results"
  style:contain="layout"
  data-search-results
>
  <ContextMenu.Root>
    <ContextMenu.Trigger class="contents">
      <table
        bind:this={tableElement}
        class="w-full table-fixed border-collapse text-sm"
        style:min-width="{tableMinWidth(visibleColumnIds)}px"
      >
        <colgroup>
          {#each visibleColumnIds as colId (colId)}
            <!-- The flex column stays unsized so surplus width collects there
                 rather than being shared out across every track. -->
            <col
              style:width={colId === FLEX_COLUMN ? undefined : `${COLUMNS[colId].width}px`}
            />
          {/each}
        </colgroup>
        <SortableHeader
          headerGroups={table.getHeaderGroups()}
          thClass="px-2 pb-1.5 text-[10px] font-semibold tracking-[0.09em] uppercase text-muted-foreground select-none"
          checkVisibility={true}
          headerClass={(id) =>
            id === "time" || id === "duration" || id === "time_end" ? "text-right" : ""}
          headerOverride={courseHeaderOverride}
        />
        {#if loading && courses.length === 0}
          <tbody>
            <!-- eslint-disable-next-line svelte/no-at-html-tags -- Static skeleton markup, no user input -->
            {@html buildSkeletonHtml(visibleColumnIds, skeletonRowCount)}
          </tbody>
        {:else if courses.length === 0 && !loading}
          <tbody>
            <tr>
              <td
                colspan={visibleColumnIds.length}
                class="py-12 text-center text-muted-foreground"
              >
                <EmptyState />
              </td>
            </tr>
          </tbody>
        {:else}
          {#each table.getRowModel().rows as row (row.id)}
            {@const course = row.original}
            {@const spansTime = timeSpansPair(course, (id) => columnVisibility[id] !== false)}
            <!-- No entry animation: the scoped view transition on [data-search-results]
                 already crossfades the whole table, and a per-row fade layered inside it
                 reads as a double flash -- including on first paint, over SSR rows that
                 were never absent. -->
            <tbody
              class="transition-opacity duration-200 {stale ? 'opacity-45 pointer-events-none' : ''}"
              animate:flip={{ duration: hadResults ? 300 : 0 }}
            >
              <tr
                class="h-10 border-b border-border/60 hover:bg-muted/40 transition-colors cursor-pointer {expandedCrn === course.crn ? 'bg-muted/30' : ''}"
                onclick={(e) => { if (!(e.target as HTMLElement).closest('a')) onToggle(course.crn); }}
              >
                {#each visibleColumnIds as colId (colId)}
                  {#if !(spansTime && colId === "time_end")}
                    {@const CellComponent = COLUMNS[colId].cell}
                    <CellComponent {course} />
                  {/if}
                {/each}
              </tr>
              {#if expandedCrn === course.crn}
                <tr>
                  <td colspan={visibleColumnIds.length} class="p-0">
                    <div transition:slide={{ duration: 200 }}>
                      <CourseDetail {course} />
                    </div>
                  </td>
                </tr>
              {/if}
            </tbody>
          {/each}
        {/if}
      </table>
    </ContextMenu.Trigger>
    <ContextMenu.Portal>
      <ContextMenu.Content
        class="z-50 min-w-40 rounded-md border border-border bg-card p-1 text-card-foreground shadow-lg"
        forceMount
      >
        {#snippet child({ wrapperProps, props, open })}
          {#if open}
            <div {...wrapperProps}>
              <div
                {...props}
                in:fade={{ duration: 100 }}
                out:fade={{ duration: 100 }}
              >
                <ContextMenu.Group>
                  <ContextMenu.GroupHeading
                    class="px-2 py-1.5 text-xs font-medium text-muted-foreground select-none"
                  >
                    Toggle columns
                  </ContextMenu.GroupHeading>
                  {#each COLUMN_DEFS as col (col.id)}
                    {@const id = col.id}
                    {@const label = col.header}
                    <ContextMenu.CheckboxItem
                      checked={columnVisibility[id] !== false}
                      closeOnSelect={false}
                      onCheckedChange={(checked) => {
                        columnVisibility = {
                          ...columnVisibility,
                          [id]: checked,
                        };
                      }}
                      class="relative flex items-center gap-2 rounded-sm px-2 py-1.5 text-sm cursor-pointer select-none outline-none data-highlighted:bg-accent data-highlighted:text-accent-foreground"
                    >
                      {#snippet children({ checked })}
                        <span
                          class="flex size-4 items-center justify-center rounded-sm border border-border"
                        >
                          {#if checked}
                            <Check class="size-3" />
                          {/if}
                        </span>
                        {label}
                      {/snippet}
                    </ContextMenu.CheckboxItem>
                  {/each}
                </ContextMenu.Group>
                {#if hasCustomVisibility}
                  <ContextMenu.Separator class="mx-1 my-1 h-px bg-border" />
                  <ContextMenu.Item
                    class="flex items-center gap-2 rounded-sm px-2 py-1.5 text-sm cursor-pointer select-none outline-none data-highlighted:bg-accent data-highlighted:text-accent-foreground"
                    onSelect={resetColumnVisibility}
                  >
                    <RotateCcw class="size-3.5" />
                    Reset to default
                  </ContextMenu.Item>
                {/if}
              </div>
            </div>
          {/if}
        {/snippet}
      </ContextMenu.Content>
    </ContextMenu.Portal>
  </ContextMenu.Root>
</div>
