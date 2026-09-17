<script lang="ts">
import { untrack } from "svelte";
import { invalidateAll } from "$app/navigation";
import { page } from "$app/state";
import { navigating } from "$app/stores";
import type { Subject } from "$lib/api";
import type { SearchOptionsResponse } from "$lib/bindings";
import ActiveFilterChips from "$lib/components/ActiveFilterChips.svelte";
import ColumnVisibilityDropdown from "$lib/components/ColumnVisibilityDropdown.svelte";
import {
  type CourseDetailContext,
  setCourseDetailContext,
} from "$lib/components/course-detail/context";
import { CourseTable } from "$lib/components/course-table";
import { COLUMN_DEFS } from "$lib/components/course-table/columns";
import Footer from "$lib/components/Footer.svelte";
import Pagination from "$lib/components/Pagination.svelte";
import SearchFiltersBar from "$lib/components/SearchFilters.svelte";
import SearchStatus from "$lib/components/SearchStatus.svelte";
import { ColumnVisibilityController } from "$lib/composables/useColumnVisibility.svelte";
import { SortController } from "$lib/composables/useSort.svelte";
import { type URLSyncHandle, useURLSync } from "$lib/composables/useURLSync.svelte";
import { PAGE_SIZE, parseFilters, searchKey } from "$lib/filters";
import { ownPayload } from "$lib/route-data";
import { parseSort } from "$lib/sort";
import { InstructorNames, setInstructorNames } from "$lib/stores/instructor-names";
import { createFilterState, setFiltersContext } from "$lib/stores/search-filters.svelte";
import type { PageProps } from "./$types";

let { data }: PageProps = $props();

let courseTableRef: { navigateToSection: (crn: string) => void } | undefined = $state();

/**
 * The payload to render from, which is the last one that was ours.
 *
 * An abandoned navigation can leave another route's data in `data`, and the
 * fields below are not on it. Holding the previous payload rather than dropping
 * to nothing keeps the results on screen through that window, and they are the
 * right results: the URL the router is on has not changed either.
 */
let held: typeof data | null = null;
const own = $derived.by(() => {
  const payload = ownPayload(data, "searchOptions");
  if (payload) held = payload;
  return payload ?? held;
});

/**
 * The route this page was mounted for, captured rather than written down, so a
 * move into a route group cannot silently leave the guard below always false.
 */
const routeId = untrack(() => page.route.id);
/** Whether the router is still on this route, so the URL is still ours to follow. */
const isCurrentRoute = $derived(page.route.id === routeId);

// Reactive derivations from load data
const searchOptions = $derived(own?.searchOptions ?? null);
const searchResult = $derived(own?.searchResult ?? null);
const searchMeta = $derived(own?.searchMeta ?? null);
const searchError = $derived(own?.searchError ?? null);
const loading = $derived($navigating !== null);

function resolveState(urlSearch: string, options: SearchOptionsResponse | null) {
  const params = new URLSearchParams(urlSearch);
  const terms = options?.terms ?? [];
  const defaultTerm = terms[0]?.slug ?? "";
  const urlTerm = params.get("term");
  return {
    params,
    selectedTerm: urlTerm && terms.some((t) => t.slug === urlTerm) ? urlTerm : defaultTerm,
    offset: Number(params.get("offset")) || 0,
    sorting: parseSort(params.get("sort")),
  };
}

// Hydrate initial filter state from URL -- intentionally one-time reads; $effect below handles re-sync
const initial = resolveState(
  untrack(() => page.url.search),
  untrack(() => data.searchOptions)
);
const validSubjects = new Set(untrack(() => data.searchOptions?.subjects.map((s) => s.code) ?? []));
const filters = createFilterState(initial.params, validSubjects);
setFiltersContext(filters);

const instructorNames = new InstructorNames(untrack(() => data.resolvedInstructors));
setInstructorNames(instructorNames);

let selectedTerm = $state(initial.selectedTerm);
let offset = $state(initial.offset);

const sort = new SortController({
  catalog: () => searchOptions?.sorts ?? [],
  initial: initial.sorting,
  onChange: () => {
    offset = 0;
    urlSync.navigateNow();
  },
});

// Re-sync mutable state on subsequent navigations. The URL comes from the
// router rather than from the load payload: the payload can be another route's,
// and its empty query string would clear every filter the URL still names.
$effect(() => {
  if (!isCurrentRoute) return;
  const resolved = resolveState(
    page.url.search,
    untrack(() => searchOptions)
  );
  const subjects = new Set(untrack(() => searchOptions?.subjects.map((s) => s.code) ?? []));
  const parsed = parseFilters(resolved.params, subjects);

  selectedTerm = resolved.selectedTerm;
  // Apply parsed filter state to the reactive object
  Object.assign(filters, parsed);
  offset = resolved.offset;
  sort.sync(resolved.sorting);
});

// Names the load resolved, kept apart from the URL sync above: they arrive with
// the payload, so there is nothing to apply while the payload is not ours.
$effect(() => {
  const resolved = own?.resolvedInstructors;
  if (resolved) instructorNames.seed(resolved);
});

const defaultTermSlug = $derived(searchOptions?.terms[0]?.slug ?? "");

const terms = $derived(searchOptions?.terms ?? []);
const subjects: Subject[] = $derived(searchOptions?.subjects ?? []);
const subjectMap: Record<string, string> = $derived(
  Object.fromEntries(subjects.map((s) => [s.code, s.description]))
);

const isSummerTerm = $derived(selectedTerm.startsWith("summer-"));

const referenceData = $derived({
  instructionalMethods: searchOptions?.reference.instructionalMethods ?? [],
  campuses: searchOptions?.reference.campuses ?? [],
  // Summer terms don't have parts of term; suppress the filter entirely
  partsOfTerm: isSummerTerm ? [] : (searchOptions?.reference.partsOfTerm ?? []),
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

const courseDetailCtx: CourseDetailContext = { navigateToSection: null };
setCourseDetailContext(courseDetailCtx);

$effect(() => {
  const table = courseTableRef;
  if (!table) return;
  courseDetailCtx.navigateToSection = (crn: string) => table.navigateToSection(crn);
});

const columns = new ColumnVisibilityController({
  autoHideColumns: ["instructor", "days", "duration"],
  // The end time is opt-in: the start plus the duration already pins a meeting,
  // and shown together the two halves render as one aligned range.
  defaultHidden: ["time_end"],
  columns: COLUMN_DEFS.map((c) => ({ id: c.id, label: c.header })),
});

// Clear part-of-term selections when switching to a summer term
$effect(() => {
  if (isSummerTerm && filters.partOfTerm.length > 0) {
    filters.partOfTerm = [];
  }
});

// Reset offset when filters change
let prevFilterKey = $state("");
$effect(() => {
  const key = searchKey(filters);
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
  sorting: () => sort.terms,
});

// The size the load actually asked for, so the pager cannot count against a
// different one.
const limit = PAGE_SIZE;

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
        <ColumnVisibilityDropdown {columns} {sort} />
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
        courses={searchResult?.items ?? []}
        {loading}
        {sort}
        {subjectMap}
        {limit}
        bind:columnVisibility={columns.visibility}
        defaultVisibility={columns.defaultVisibility}
      />

      {#if searchResult}
        <Pagination
          totalCount={searchResult.total}
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
