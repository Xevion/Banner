<script lang="ts">
import type { VisibilityState } from "@tanstack/table-core";
import type { CourseResponse } from "$lib/bindings";
import type { SortController } from "$lib/composables/useSort.svelte";
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
  columnVisibility?: VisibilityState;
  /** What "reset to default" restores, for columns hidden until opted into. */
  defaultVisibility?: VisibilityState;
} = $props();

const state = useCourseTableState(
  () => courses,
  () => limit,
  () => loading
);

export function navigateToSection(crn: string) {
  state.toggleRow(crn);
}
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
