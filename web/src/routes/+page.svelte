<script lang="ts">
import type { SearchOptionsResponse } from "$lib/bindings";
import { type SearchResponse, type Subject, client } from "$lib/api";
import { CourseTable } from "$lib/components/course-table";
import {
  buildAttributeMap,
  setCourseDetailContext,
  type CourseDetailContext,
} from "$lib/components/course-detail/context";
import ActiveFilterChips from "$lib/components/ActiveFilterChips.svelte";
import ColumnVisibilityDropdown from "$lib/components/ColumnVisibilityDropdown.svelte";
import Footer from "$lib/components/Footer.svelte";
import Pagination from "$lib/components/Pagination.svelte";
import SearchFiltersBar from "$lib/components/SearchFilters.svelte";
import SearchStatus from "$lib/components/SearchStatus.svelte";
import type { SortingState } from "@tanstack/table-core";
import { tick, untrack } from "svelte";
import { useURLSync } from "$lib/composables/useURLSync.svelte";
import { ColumnVisibilityController } from "$lib/composables/useColumnVisibility.svelte";
import { SearchFilters, setFiltersContext } from "$lib/stores/search-filters.svelte";

interface PageLoadData {
  searchOptions: SearchOptionsResponse | null;
  url: URL;
}

let { data }: { data: PageLoadData } = $props();

/** No-op function to register Svelte reactivity dependencies for `$effect` tracking */
function track(..._deps: unknown[]) {
  /* noop */
}

let courseTableRef: { navigateToSection: (crn: string) => void } | undefined = $state();

const initialParams = untrack(() => new URLSearchParams(data.url.search));
const defaultTermSlug = untrack(() => data.searchOptions?.terms[0]?.slug ?? "");

const urlTerm = initialParams.get("term");
let selectedTerm = $state(
  untrack(() => {
    const terms = data.searchOptions?.terms ?? [];
    return urlTerm && terms.some((t) => t.slug === urlTerm) ? urlTerm : defaultTermSlug;
  })
);

const filters = new SearchFilters();
setFiltersContext(filters);
untrack(() => {
  const validSubjects = new Set(data.searchOptions?.subjects.map((s) => s.code));
  filters.fromURLParams(initialParams, validSubjects);
});

let offset = $state(Number(initialParams.get("offset")) || 0);
const limit = 25;

// svelte-ignore state_referenced_locally
let searchOptions = $state<SearchOptionsResponse | null>(data.searchOptions);

let sorting: SortingState = $state(
  (() => {
    const sortBy = initialParams.get("sort_by");
    const sortDir = initialParams.get("sort_dir");
    if (!sortBy) return [];
    return [{ id: sortBy, desc: sortDir === "desc" }];
  })()
);

const terms = $derived(searchOptions?.terms ?? []);
const subjects: Subject[] = $derived(searchOptions?.subjects ?? []);
const subjectMap: Record<string, string> = $derived(
  Object.fromEntries(subjects.map((s) => [s.code, s.description]))
);

const referenceData = $derived({
  instructionalMethods: searchOptions?.reference.instructionalMethods ?? [],
  campuses: searchOptions?.reference.campuses ?? [],
  partsOfTerm: searchOptions?.reference.partsOfTerm ?? [],
  attributes: searchOptions?.reference.attributes ?? [],
});

const ranges = $derived(
  searchOptions?.ranges ?? {
    courseNumberMin: 0,
    courseNumberMax: 9000,
    creditHourMin: 0,
    creditHourMax: 8,
    waitCountMax: 0,
  }
);

const attributeMap = $derived(buildAttributeMap(referenceData.attributes));
const courseDetailCtx: CourseDetailContext = {
  get attributeMap() {
    return attributeMap;
  },
  navigateToSection: null,
};
setCourseDetailContext(courseDetailCtx);

$effect(() => {
  if (courseTableRef) {
    courseDetailCtx.navigateToSection = (crn: string) => courseTableRef?.navigateToSection(crn);
  }
});

const columns = new ColumnVisibilityController({
  autoHideColumns: ["crn", "location"],
  columns: [
    { id: "crn", label: "CRN" },
    { id: "course_code", label: "Course" },
    { id: "title", label: "Title" },
    { id: "instructor", label: "Instructor" },
    { id: "time", label: "Time" },
    { id: "location", label: "Location" },
    { id: "seats", label: "Seats" },
  ],
});

// Re-sync state from URL on browser back/forward navigation.
// SvelteKit re-runs the load function and updates `data`, but component
// state was initialized once — this effect bridges the gap.
let prevUrlSearch = untrack(() => data.url.search);
$effect(() => {
  const search = data.url.search;
  if (search === prevUrlSearch) return;
  prevUrlSearch = search;

  const params = new URLSearchParams(search);
  const validSubjects = new Set(untrack(() => searchOptions?.subjects.map((s) => s.code) ?? []));

  const urlTerm = params.get("term");
  const termList = untrack(() => searchOptions?.terms ?? []);
  selectedTerm = urlTerm && termList.some((t) => t.slug === urlTerm) ? urlTerm : defaultTermSlug;

  filters.fromURLParams(params, validSubjects);
  offset = Number(params.get("offset")) || 0;

  const sortBy = params.get("sort_by");
  const sortDir = params.get("sort_dir");
  sorting = sortBy ? [{ id: sortBy, desc: sortDir === "desc" }] : [];
});

// Keep URL params in sync with filter state
useURLSync({
  filters,
  selectedTerm: () => selectedTerm,
  defaultTermSlug,
  offset: () => offset,
  sorting: () => sorting,
});

let searchResult: SearchResponse | null = $state(null);
let searchMeta: { totalCount: number; durationMs: number; timestamp: Date } | null = $state(null);
let loading = $state(false);
let error = $state<string | null>(null);

let validatingSubjects = false;
let searchTimeout: ReturnType<typeof setTimeout> | undefined;
let lastSearchKey = "";
let fetchCounter = 0;

// Fetch new search options when term changes
$effect(() => {
  const term = selectedTerm;
  if (!term) return;
  void client.getSearchOptions(term).then((result) => {
    if (result.isErr) {
      console.error("Failed to fetch search options:", result.error);
      return;
    }
    const opts = result.value;
    searchOptions = opts;
    const validCodes = new Set(opts.subjects.map((s) => s.code));
    const filtered = filters.subject.filter((code) => validCodes.has(code));
    if (filtered.length !== filters.subject.length) {
      validatingSubjects = true;
      filters.subject = filtered;
      validatingSubjects = false;
    }
  });
});

// Unified search effect
$effect(() => {
  const term = selectedTerm;
  const filterKey = filters.toSearchKey();
  track(offset, sorting);

  if (validatingSubjects) return;

  const searchKey = [term, filterKey, offset, JSON.stringify(sorting)].join("|");
  const THROTTLE_MS = 300;
  clearTimeout(searchTimeout);
  searchTimeout = setTimeout(() => {
    if (searchKey === lastSearchKey) return;
    void performSearch();
  }, THROTTLE_MS);

  return () => clearTimeout(searchTimeout);
});

// Reset offset when filters change
let prevFilters = $state("");
$effect(() => {
  const key = filters.toSearchKey();
  if (prevFilters && key !== prevFilters) {
    offset = 0;
  }
  prevFilters = key;
});

async function performSearch() {
  if (!selectedTerm) return;
  const key = [selectedTerm, filters.toSearchKey(), offset, JSON.stringify(sorting)].join("|");
  lastSearchKey = key;
  loading = true;
  error = null;

  const fetchId = ++fetchCounter;
  const t0 = performance.now();
  const apiParams = filters.toAPIParams(selectedTerm, limit, offset, sorting);
  const result = await client.searchCourses(apiParams);

  // Ignore stale responses
  if (fetchId !== fetchCounter) return;

  if (result.isErr) {
    error = result.error.message;
    loading = false;
    return;
  }

  const data = result.value;
  const applyUpdate = () => {
    searchResult = data;
    searchMeta = {
      totalCount: data.totalCount,
      durationMs: performance.now() - t0,
      timestamp: new Date(),
    };
  };

  // Scoped view transitions only affect the table element
  const tableEl = document.querySelector("[data-search-results]");
  if (tableEl && "startViewTransition" in tableEl) {
    const startViewTransition = (
      tableEl as unknown as {
        startViewTransition: (cb: () => Promise<void>) => {
          updateCallbackDone: Promise<void>;
        };
      }
    ).startViewTransition;
    const transition = startViewTransition(async () => {
      applyUpdate();
      await tick();
    });
    await transition.updateCallbackDone;
  } else {
    applyUpdate();
  }

  loading = false;
}

function handleSortingChange(newSorting: SortingState) {
  sorting = newSorting;
  offset = 0;
}

function handlePageChange(newOffset: number) {
  offset = newOffset;
}
</script>

<svelte:head>
  <title>Course Search | Banner</title>
</svelte:head>

<div class="min-h-screen flex flex-col items-center px-3 md:px-5 pb-5 pt-20">
  <div class="w-full max-w-6xl flex flex-col pt-2">
    <!-- Chips bar: status | chips | view button -->
    <div class="flex flex-col md:flex-row md:items-end gap-1 md:gap-3 min-h-7">
      <SearchStatus meta={searchMeta} {loading} />
      <ActiveFilterChips {filters} />
      <div class="hidden md:block pb-1.5">
        <ColumnVisibilityDropdown {columns} />
      </div>
    </div>

    <!-- Filter bar -->
    <div class="flex flex-col gap-2 pb-4">
      <SearchFiltersBar
        {terms}
        {subjects}
        bind:selectedTerm
        {referenceData}
        ranges={{
          courseNumber: { min: ranges.courseNumberMin, max: ranges.courseNumberMax },
          creditHours: { min: ranges.creditHourMin, max: ranges.creditHourMax },
          waitCount: { max: ranges.waitCountMax },
        }}
      />
    </div>

    <!-- Results -->
    {#if error}
      <div class="text-center py-8">
        <p class="text-status-red">{error}</p>
        <button
          onclick={() => performSearch()}
          class="mt-2 text-sm text-muted-foreground hover:underline"
        >
          Retry
        </button>
      </div>
    {:else}
      <CourseTable
        bind:this={courseTableRef}
        courses={searchResult?.courses ?? []}
        {loading}
        {sorting}
        onSortingChange={handleSortingChange}
        manualSorting={true}
        {subjectMap}
        {limit}
        bind:columnVisibility={columns.visibility}
      />

      {#if searchResult}
        <Pagination
          totalCount={searchResult.totalCount}
          {offset}
          {limit}
          {loading}
          onPageChange={handlePageChange}
        />
      {/if}
    {/if}

    <!-- Footer -->
    <Footer />
  </div>
</div>
