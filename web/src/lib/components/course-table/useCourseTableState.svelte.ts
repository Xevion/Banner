// useCourseTableState.svelte.ts
import type { CourseResponse } from "$lib/bindings";

export function useCourseTableState(getCourses: () => CourseResponse[], getLimit: () => number) {
  let expandedCrn: string | null = $state(null);
  let previousRowCount = $state(0);
  let hadResults = $state(false);
  let contentHeight = $state<number | null>(null);

  // Track previous row count so skeleton matches expected result size
  $effect(() => {
    const courses = getCourses();
    if (courses.length > 0) {
      previousRowCount = courses.length;
    }
  });

  const skeletonRowCount = $derived(previousRowCount > 0 ? previousRowCount : getLimit());

  // Collapse expanded row when dataset changes
  $effect(() => {
    getCourses(); // track dependency
    expandedCrn = null;
  });

  // Skip FLIP on initial load
  $effect(() => {
    if (getCourses().length > 0) hadResults = true;
  });

  function toggleRow(crn: string) {
    expandedCrn = expandedCrn === crn ? null : crn;
  }

  /** Bind to the table element to track content height via ResizeObserver */
  function observeHeight(tableElement: HTMLTableElement) {
    const observer = new ResizeObserver(([entry]) => {
      contentHeight = entry.contentRect.height;
    });
    observer.observe(tableElement);
    return () => observer.disconnect();
  }

  return {
    get expandedCrn() {
      return expandedCrn;
    },
    get skeletonRowCount() {
      return skeletonRowCount;
    },
    get hadResults() {
      return hadResults;
    },
    get contentHeight() {
      return contentHeight;
    },
    toggleRow,
    observeHeight,
  };
}
