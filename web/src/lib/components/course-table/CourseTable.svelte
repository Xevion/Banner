<script lang="ts">
import type { CourseResponse } from "$lib/bindings";
import type { SortingState, VisibilityState } from "@tanstack/table-core";
import CourseTableDesktop from "./CourseTableDesktop.svelte";
import CourseTableMobile from "./CourseTableMobile.svelte";
import { useCourseTableState } from "./useCourseTableState.svelte";

let {
  courses,
  loading,
  sorting = [],
  onSortingChange,
  manualSorting = false,
  subjectMap = {},
  limit = 25,
  columnVisibility = $bindable({}),
}: {
  courses: CourseResponse[];
  loading: boolean;
  sorting?: SortingState;
  onSortingChange?: (sorting: SortingState) => void;
  manualSorting?: boolean;
  subjectMap?: Record<string, string>;
  limit?: number;
  columnVisibility?: VisibilityState;
} = $props();

const state = useCourseTableState(
  () => courses,
  () => limit
);

export function navigateToSection(crn: string) {
  state.toggleRow(crn);
}
</script>

<CourseTableMobile
  {courses}
  {loading}
  skeletonRowCount={state.skeletonRowCount}
  expandedCrn={state.expandedCrn}
  onToggle={state.toggleRow}
/>

<CourseTableDesktop
  {courses}
  {loading}
  {sorting}
  {onSortingChange}
  {manualSorting}
  {subjectMap}
  bind:columnVisibility
  expandedCrn={state.expandedCrn}
  onToggle={state.toggleRow}
  skeletonRowCount={state.skeletonRowCount}
  hadResults={state.hadResults}
  observeHeight={state.observeHeight}
  contentHeight={state.contentHeight}
/>
