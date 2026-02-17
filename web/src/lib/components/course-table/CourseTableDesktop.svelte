<script lang="ts">
import type { CourseResponse } from "$lib/bindings";
import { FlexRender, createSvelteTable } from "$lib/components/ui/data-table/index.js";
import { useClipboard } from "$lib/composables/useClipboard.svelte";
import { useOverlayScrollbars } from "$lib/composables/useOverlayScrollbars.svelte";
import { useTooltipDelegation } from "$lib/composables/useTooltipDelegation";
import { ArrowDown, ArrowUp, ArrowUpDown, Check, RotateCcw } from "@lucide/svelte";
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
import { setContext } from "svelte";
import CourseDetail from "$lib/components/CourseDetail.svelte";
import { COLUMN_DEFS, CELL_COMPONENTS } from "./columns";
import { buildSkeletonHtml } from "./skeletons";
import EmptyState from "./EmptyState.svelte";
import { TABLE_CONTEXT_KEY } from "./context";

let {
  courses,
  loading,
  sorting = [],
  onSortingChange,
  manualSorting = false,
  subjectMap = {},
  columnVisibility = $bindable({}),
  expandedCrn,
  onToggle,
  skeletonRowCount,
  hadResults,
  observeHeight,
  contentHeight,
}: {
  courses: CourseResponse[];
  loading: boolean;
  sorting?: SortingState;
  onSortingChange?: (sorting: SortingState) => void;
  manualSorting?: boolean;
  subjectMap?: Record<string, string>;
  columnVisibility?: VisibilityState;
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
setContext(TABLE_CONTEXT_KEY, {
  clipboard,
  get subjectMap() {
    return subjectMap;
  },
  get maxSubjectLength() {
    return maxSubjectLength;
  },
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

let hasCustomVisibility = $derived(Object.values(columnVisibility).some((v) => v === false));

function resetColumnVisibility() {
  columnVisibility = {};
}

function handleVisibilityChange(updater: Updater<VisibilityState>) {
  const newVisibility = typeof updater === "function" ? updater(columnVisibility) : updater;
  columnVisibility = newVisibility;
}

function handleSortingChange(updater: Updater<SortingState>) {
  const newSorting = typeof updater === "function" ? updater(sorting) : updater;
  onSortingChange?.(newSorting);
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
      <table bind:this={tableElement} class="w-full min-w-120 md:min-w-160 border-collapse text-sm">
        <thead>
          {#each table.getHeaderGroups() as headerGroup (headerGroup.id)}
            <tr class="border-b border-border text-left text-muted-foreground">
              {#each headerGroup.headers as header (header.id)}
                {#if header.column.getIsVisible()}
                  <th
                    class="py-2 px-2 font-medium select-none {header.id === 'seats' ? 'text-right' : ''}"
                    class:cursor-pointer={header.column.getCanSort()}
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
                {/if}
              {/each}
            </tr>
          {/each}
        </thead>
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
            <tbody
              class="transition-opacity duration-200 animate-fade-in {loading ? 'opacity-45 pointer-events-none' : ''}"
              animate:flip={{ duration: hadResults ? 300 : 0 }}
            >
              <tr
                class="border-b border-border cursor-pointer hover:bg-muted/50 transition-colors whitespace-nowrap {expandedCrn === course.crn ? 'bg-muted/30' : ''}"
                onclick={() => onToggle(course.crn)}
              >
                {#each visibleColumnIds as colId (colId)}
                  {@const CellComponent = CELL_COMPONENTS[colId]}
                  {#if CellComponent}
                    <CellComponent {course} />
                  {:else}
                    <td class="py-2 px-2 text-muted-foreground">—</td>
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
