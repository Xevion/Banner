<script lang="ts">
import type { ColumnVisibilityState } from "@tanstack/table-core";
import type { CourseResponse } from "$lib/bindings";
import {
  COLUMN_DEFS,
  COLUMNS,
  FLEX_COLUMN,
  tableMinWidth,
} from "$lib/components/course-table/columns";
import { setTableContext } from "$lib/components/course-table/context";
import { courseHeaderOverride } from "$lib/components/course-table/header-override";
import SortableHeader from "$lib/components/SortableHeader.svelte";
import { APP_TABLE_FEATURES, createSvelteTable } from "$lib/components/ui/data-table/index.js";
import { useClipboard } from "$lib/composables/useClipboard.svelte";
import { SortController } from "$lib/composables/useSort.svelte";
import { timeSpansPair } from "$lib/scheduleState";
import { parseSort } from "$lib/sort";
import { sortCatalog } from "./fixtures/sort";

let {
  courses = [],
  sort: initialSort = "",
  // End Time starts hidden, as it does on the search page.
  columnVisibility = { time_end: false },
  subjectMap = {},
}: {
  courses?: CourseResponse[];
  /** Wire format, as the `sort` search param carries it. */
  sort?: string;
  columnVisibility?: ColumnVisibilityState;
  subjectMap?: Record<string, string>;
} = $props();

// Live rather than fixed, so a story can be clicked through the whole cycle.
// Derived, not constructed once: changing the arg reseeds it, clicking does not.
const sort = $derived(
  new SortController({ catalog: () => sortCatalog, initial: parseSort(initialSort) })
);

const maxSubjectLength = $derived(
  courses.length > 0 ? Math.max(...courses.map((c) => c.subject.length)) : 3
);

setTableContext({
  clipboard: useClipboard(1000),
  get subjectMap() {
    return subjectMap;
  },
  get maxSubjectLength() {
    return maxSubjectLength;
  },
  isColumnVisible: (id: string) => columnVisibility[id] !== false,
});

const visibleColumnIds = $derived(
  COLUMN_DEFS.map((c) => c.id).filter((id) => columnVisibility[id] !== false)
);

const sortLabels = new Map(sortCatalog.map((option) => [option.key, option]));

const headerOverride = (headerId: string) =>
  courseHeaderOverride(headerId, {
    terms: sort.terms,
    labels: sortLabels,
    isColumnVisible: (id) => columnVisibility[id] !== false,
    onSort: (next) => sort.applyHeaderClick(next),
  });

const table = createSvelteTable({
  features: APP_TABLE_FEATURES,
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
});
</script>

<!-- The track widths and the wrapper's scroll are what the headers have to fit
     inside, so the story reproduces the desktop shell rather than the header alone. -->
<div class="w-full overflow-x-auto">
  <table
    class="w-full table-fixed border-collapse text-sm"
    style:min-width="{tableMinWidth(visibleColumnIds)}px"
  >
    <colgroup>
      {#each visibleColumnIds as colId (colId)}
        <col style:width={colId === FLEX_COLUMN ? undefined : `${COLUMNS[colId].width}px`} />
      {/each}
    </colgroup>
    <SortableHeader
      headerGroups={table.getHeaderGroups()}
      thClass="px-2 pb-1.5 text-[10px] font-semibold tracking-[0.09em] uppercase text-muted-foreground select-none"
      checkVisibility={true}
      headerClass={(id) =>
        id === "time" || id === "duration" || id === "time_end" ? "text-right" : ""}
      {headerOverride}
    />
    {#each table.getRowModel().rows as row (row.id)}
      {@const course = row.original}
      {@const spansTime = timeSpansPair(course, (id) => columnVisibility[id] !== false)}
      <tbody>
        <tr class="h-10 border-b border-border/60">
          {#each visibleColumnIds as colId (colId)}
            {#if !(spansTime && colId === "time_end")}
              {@const CellComponent = COLUMNS[colId].cell}
              <CellComponent {course} />
            {/if}
          {/each}
        </tr>
      </tbody>
    {/each}
  </table>
</div>
