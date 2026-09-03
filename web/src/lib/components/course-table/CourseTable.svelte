<script lang="ts">
import type { CourseResponse } from "$lib/bindings";
import type { SortKeyOption } from "$lib/bindings";
import type { SortTerm } from "$lib/sort";
import type { VisibilityState } from "@tanstack/table-core";
import CourseTableDesktop from "./CourseTableDesktop.svelte";
import CourseTableMobile from "./CourseTableMobile.svelte";
import { useCourseTableState } from "./useCourseTableState.svelte";

let {
  courses,
  loading,
  sorting = [],
  sortOptions = [],
  onSortingChange,
  subjectMap = {},
  limit = 25,
  columnVisibility = $bindable({}),
  defaultVisibility = {},
}: {
  courses: CourseResponse[];
  loading: boolean;
  sorting?: SortTerm[];
  sortOptions?: SortKeyOption[];
  onSortingChange?: (sorting: SortTerm[]) => void;
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
  {sorting}
  {sortOptions}
  {onSortingChange}
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
