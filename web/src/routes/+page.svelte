<script lang="ts">
import type { SearchOptionsResponse } from "$lib/bindings";
import type { SearchResponse, Subject } from "$lib/api";
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
import { untrack } from "svelte";
import { useURLSync, type URLSyncHandle } from "$lib/composables/useURLSync.svelte";
import { ColumnVisibilityController } from "$lib/composables/useColumnVisibility.svelte";
import { SearchFilters, setFiltersContext } from "$lib/stores/search-filters.svelte";
import { navigating } from "$app/stores";
import { invalidateAll } from "$app/navigation";

interface PageLoadData {
  searchOptions: SearchOptionsResponse | null;
  searchResult: SearchResponse | null;
  searchError: string | null;
  searchMeta: { totalCount: number; durationMs: number; timestamp: Date } | null;
  url: URL;
}

let { data }: { data: PageLoadData } = $props();

let courseTableRef: { navigateToSection: (crn: string) => void } | undefined = $state();

// Reactive derivations from load data
const searchOptions = $derived(data.searchOptions);
const searchResult = $derived(data.searchResult);
const searchMeta = $derived(data.searchMeta);
const searchError = $derived(data.searchError);
const loading = $derived($navigating !== null);

// Mutable filter state — UI's writable handle, synced to URL
const filters = new SearchFilters();
setFiltersContext(filters);

let selectedTerm = $state("");
let offset = $state(0);
let sorting: SortingState = $state([]);

// Hydrate mutable state from load data on every navigation
$effect(() => {
  const params = new URLSearchParams(data.url.search);
  const validSubjects = new Set(untrack(() => searchOptions?.subjects.map((s) => s.code) ?? []));

  const urlTerm = params.get("term");
  const termList = untrack(() => searchOptions?.terms ?? []);
  const defaultTerm = untrack(() => searchOptions?.terms[0]?.slug ?? "");
  selectedTerm = urlTerm && termList.some((t) => t.slug === urlTerm) ? urlTerm : defaultTerm;

  filters.fromURLParams(params, validSubjects);
  offset = Number(params.get("offset")) || 0;

  const sortBy = params.get("sort_by");
  const sortDir = params.get("sort_dir");
  sorting = sortBy ? [{ id: sortBy, desc: sortDir === "desc" }] : [];
});

const defaultTermSlug = $derived(searchOptions?.terms[0]?.slug ?? "");

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

// Reset offset when filters change
let prevFilterKey = $state("");
$effect(() => {
  const key = filters.toSearchKey();
  if (prevFilterKey && key !== prevFilterKey) {
    offset = 0;
  }
  prevFilterKey = key;
});

// Keep URL in sync with filter state; debounces text input, immediate for discrete changes
const urlSync: URLSyncHandle = useURLSync({
  filters,
  selectedTerm: () => selectedTerm,
  defaultTermSlug: () => defaultTermSlug,
  offset: () => offset,
  sorting: () => sorting,
});

const limit = 25;

function handleSortingChange(newSorting: SortingState) {
  sorting = newSorting;
  offset = 0;
  urlSync.navigateNow();
}

function handlePageChange(newOffset: number) {
  offset = newOffset;
  urlSync.navigateNow();
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
    {#if searchError}
      <div class="text-center py-8">
        <p class="text-status-red">{searchError}</p>
        <button
          onclick={() => invalidateAll()}
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
