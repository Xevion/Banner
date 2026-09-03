<script lang="ts">
import type { CourseResponse } from "$lib/bindings";
import CourseDetail from "$lib/components/CourseDetail.svelte";
import SortableHeader, { type HeaderOverride } from "$lib/components/SortableHeader.svelte";
import { createSvelteTable } from "$lib/components/ui/data-table/index.js";
import { useClipboard } from "$lib/composables/useClipboard.svelte";
import { useOverlayScrollbars } from "$lib/composables/useOverlayScrollbars.svelte";
import { useTooltipDelegation } from "$lib/composables/useTooltipDelegation";
import { createSortingHandler } from "$lib/composables/sorting";
import { Check, RotateCcw } from "@lucide/svelte";
import {
  type SortingState,
  type Updater,
  type VisibilityState,
  getCoreRowModel,
  getSortedRowModel,
} from "@tanstack/table-core";
import { ContextMenu } from "bits-ui";
import { flip } from "svelte/animate";
import { fade, slide } from "svelte/transition";
import EmptyState from "./EmptyState.svelte";
import { CELL_COMPONENTS, COLUMN_DEFS, COLUMN_WIDTHS, FLEX_COLUMN, tableMinWidth } from "./columns";
import { instructorSortLabel, instructorSortStep, nextInstructorSorting } from "./instructorSort";
import { setTableContext } from "./context";
import { buildSkeletonHtml } from "./skeletons";

let {
  courses,
  loading,
  stale,
  sorting = [],
  onSortingChange,
  manualSorting = false,
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
  sorting?: SortingState;
  onSortingChange?: (sorting: SortingState) => void;
  manualSorting?: boolean;
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
  COLUMN_DEFS.map((c) => c.id!).filter((id) => columnVisibility[id] !== false)
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

const handleSortingChange = createSortingHandler(
  () => sorting,
  (next) => {
    onSortingChange?.(next);
  }
);

/**
 * The instructor header cycles name and rating together, so it drives a sort key
 * that is not its own column and cannot use TanStack's per-column toggle.
 */
function courseHeaderOverride(headerId: string): HeaderOverride | null {
  if (headerId === "instructor") {
    const step = instructorSortStep(sorting);
    return {
      suffix: instructorSortLabel(sorting),
      indicator: step.indicator,
      title: step.next,
      onclick: () => onSortingChange?.(nextInstructorSorting(sorting)),
    };
  }
  // Shown together the two columns are one range, so they carry one label over
  // the pair rather than naming halves the reader can already see.
  if (headerId === "time" && columnVisibility.time_end !== false) return { label: "Time" };
  if (headerId === "time_end" && columnVisibility.time !== false) return { label: "" };
  return null;
}

const table = createSvelteTable({
  get data() {
    return courses;
  },
  getRowId: (row) => String(row.crn),
  columns: COLUMN_DEFS,
  state: {
    get sorting() {
      return sorting;
    },
    get columnVisibility() {
      return columnVisibility;
    },
  },
  onSortingChange: handleSortingChange,
  onColumnVisibilityChange: handleVisibilityChange,
  getCoreRowModel: getCoreRowModel(),
  get getSortedRowModel() {
    return manualSorting ? undefined : getSortedRowModel<CourseResponse>();
  },
  get manualSorting() {
    return manualSorting;
  },
  enableSortingRemoval: true,
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
              style:width={colId === FLEX_COLUMN ? undefined : `${COLUMN_WIDTHS[colId]}px`}
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
                  {@const CellComponent = CELL_COMPONENTS[colId]}
                  <CellComponent {course} />
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
                    {@const id = col.id!}
                    {@const label = typeof col.header === "string" ? col.header : id}
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
