<script lang="ts">
import type { ColumnVisibilityState } from "@tanstack/table-core";
import type { CourseResponse } from "$lib/bindings";
import type { SortController } from "$lib/composables/useSort.svelte";
import { courseTrends } from "$lib/stores/course-trends.svelte";
import CourseTableDesktop from "./CourseTableDesktop.svelte";
import CourseTableMobile from "./CourseTableMobile.svelte";
import { useCourseTableState } from "./useCourseTableState.svelte";

let {
  courses,
  loading,
  sort,
  subjectMap = {},
  limit = 25,
  columnVisibility = $bindable({}),
  defaultVisibility = {},
}: {
  courses: CourseResponse[];
  loading: boolean;
  /** Omitted where the table is a plain listing, leaving header clicks inert. */
  sort?: SortController;
  subjectMap?: Record<string, string>;
  limit?: number;
  columnVisibility?: ColumnVisibilityState;
  /** What "reset to default" restores, for columns hidden until opted into. */
  defaultVisibility?: ColumnVisibilityState;
} = $props();

const state = useCourseTableState(
  () => courses,
  () => limit,
  () => loading
);

export function navigateToSection(crn: string) {
  state.toggleRow(crn);
}

// One batched request per page of results, after the rows themselves are on screen.
$effect(() => {
  const term = courses[0]?.termSlug;
  if (!term) return;
  const crns = courses.filter((c) => c.termSlug === term).map((c) => c.crn);
  void courseTrends.load(term, crns);
});
</script>

<CourseTableMobile
  {courses}
  {loading}
  stale={state.stale}
  skeletonRowCount={state.skeletonRowCount}
  expandedCrn={state.expandedCrn}
  onToggle={state.toggleRow}
/>

<CourseTableDesktop
  {courses}
  {loading}
  stale={state.stale}
  {sort}
  {subjectMap}
  bind:columnVisibility
  {defaultVisibility}
  expandedCrn={state.expandedCrn}
  onToggle={state.toggleRow}
  skeletonRowCount={state.skeletonRowCount}
  hadResults={state.hadResults}
  observeHeight={state.observeHeight}
  contentHeight={state.contentHeight}
/>
