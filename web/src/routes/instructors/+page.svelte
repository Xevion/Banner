<script lang="ts">
import { goto } from "$app/navigation";
import { client } from "$lib/api";
import { useQuery } from "$lib/composables";
import { formatInstructorName, NameFormat } from "$lib/course";
import Breadcrumb from "$lib/components/Breadcrumb.svelte";
import Footer from "$lib/components/Footer.svelte";
import InstructorCard from "$lib/components/InstructorCard.svelte";
import SubjectCombobox from "$lib/components/SubjectCombobox.svelte";
import SortSelect from "$lib/components/SortSelect.svelte";
import type { SortOption } from "$lib/components/SortSelect.svelte";
import { compact, formatNumber, range } from "$lib/utils";
import { Search } from "@lucide/svelte";
import { untrack } from "svelte";
import type { PageProps } from "./$types";

let { data }: PageProps = $props();

let search = $state(untrack(() => data.url.searchParams.get("search") ?? ""));
let selectedSubjects = $state<string[]>(
  untrack(() => {
    const subject = data.url.searchParams.get("subject");
    return subject ? [subject] : [];
  })
);
let selectedSort = $state(untrack(() => data.url.searchParams.get("sort") ?? "name_asc"));
let page = $state(untrack(() => Number(data.url.searchParams.get("page")) || 1));

const subjects = $derived(data.searchOptions?.subjects ?? []);
const subjectMap = $derived(
  new Map(subjects.map((s: { code: string; description: string }) => [s.code, s.description]))
);

const sortOptions: SortOption[] = [
  { value: "name", label: "Alphabetical", defaultDirection: "asc" },
  { value: "score", label: "Score", defaultDirection: "desc" },
];

const query = useQuery({
  fetcher: () =>
    client.getInstructors(
      compact({
        search: search || undefined,
        subject: selectedSubjects[0],
        sort: selectedSort,
        page,
      })
    ),
  deps: () => [search, selectedSubjects[0], selectedSort, page],
  debounce: 300,
  initial: untrack(() => data.instructors),
});

const totalPages = $derived(query.data ? Math.ceil(query.data.total / query.data.perPage) : 0);

// Reset to page 1 when subject or sort changes
let _prevSubject = $state(untrack(() => selectedSubjects[0]));
$effect(() => {
  const s = selectedSubjects[0]; // tracked
  if (s !== _prevSubject) {
    _prevSubject = s;
    page = 1;
  }
});

let _prevSort = $state(untrack(() => selectedSort));
$effect(() => {
  const s = selectedSort; // tracked
  if (s !== _prevSort) {
    _prevSort = s;
    page = 1;
  }
});

// Sync filters to URL
$effect(() => {
  // eslint-disable-next-line svelte/prefer-svelte-reactivity -- built for the URL, never stored
  const params = new URLSearchParams();
  if (search) params.set("search", search);
  const onlySubject = selectedSubjects.length === 1 ? selectedSubjects[0] : undefined;
  if (onlySubject) params.set("subject", onlySubject);
  if (selectedSort !== "name_asc") params.set("sort", selectedSort);
  if (page > 1) params.set("page", String(page));
  const qs = params.toString();
  void goto(`/instructors${qs ? `?${qs}` : ""}`, { replaceState: true, keepFocus: true });
});

function resolveSubject(code: string): string {
  return subjectMap.get(code) ?? code;
}
</script>

<svelte:head>
  <title>Instructor Directory | Banner</title>
</svelte:head>

<div class="min-h-screen flex flex-col items-center px-3 md:px-5 pb-5 pt-20">
  <div class="w-full max-w-6xl flex flex-col pt-2">
    <Breadcrumb items={[{ label: "Home", href: "/" }, { label: "Instructors" }]} />
    <h1 class="text-2xl font-bold mb-4">Instructor Directory</h1>

    <!-- Filters -->
    <div class="flex flex-wrap items-end gap-2 mb-4">
      <div class="relative flex-1 min-w-[200px]">
        <Search class="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-muted-foreground" />
        <input
          type="text"
          placeholder="Search instructors..."
          bind:value={search}
          class="w-full h-9 pl-9 pr-3 text-sm rounded-md border border-border bg-card
                 focus:outline-none focus:ring-2 focus:ring-ring"
        />
      </div>

      <SubjectCombobox {subjects} bind:value={selectedSubjects} />

      <SortSelect options={sortOptions} bind:value={selectedSort} />
    </div>

    <!-- Results count -->
    {#if query.data && !query.isLoading}
      <p class="text-xs text-muted-foreground mb-3">
        {formatNumber(query.data.total)} instructor{query.data.total !== 1 ? "s" : ""} found
      </p>
    {/if}

    <!-- Card grid -->
    {#if query.isLoading && !query.data}
      <!-- Skeleton grid for initial load -->
      <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {#each range(12) as i (i)}
          <div class="rounded-lg border border-border bg-card p-4 animate-pulse">
            <div class="flex items-start justify-between gap-2">
              <div class="min-w-0 flex-1 space-y-2">
                <div class="h-4 w-32 bg-muted rounded"></div>
                <div class="h-3 w-40 bg-muted rounded"></div>
              </div>
              <div class="h-5 w-10 bg-muted rounded"></div>
            </div>
            <div class="flex gap-1 mt-2.5">
              <div class="h-5 w-20 bg-muted rounded"></div>
              <div class="h-5 w-16 bg-muted rounded"></div>
            </div>
          </div>
        {/each}
      </div>
    {:else}
      <div
        class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 transition-opacity duration-150"
        class:opacity-50={query.isLoading}
      >
        {#if query.data}
          {#each query.data.items as instructor (instructor.id)}
            <InstructorCard
              variant="grid"
              name={formatInstructorName(instructor.displayName, NameFormat.LastNameFirst)}
              slug={instructor.slug}
              email={instructor.email}
              subjects={instructor.subjects}
              subjectLabel={resolveSubject}
              rating={instructor.rating}
            />
          {/each}
        {/if}
      </div>
    {/if}

    <!-- Empty state -->
    {#if query.data?.items.length === 0 && !query.isLoading}
      <div class="text-center py-16 text-muted-foreground">
        <p class="text-sm">No instructors found matching your criteria.</p>
      </div>
    {/if}

    <!-- Pagination -->
    {#if query.data && totalPages > 1}
      <div class="flex justify-center items-center gap-2 mt-6 text-sm">
        <button
          class="px-3 py-1.5 rounded-md border border-border bg-card text-sm
                 hover:bg-muted/50 transition-colors disabled:opacity-40 disabled:pointer-events-none"
          disabled={page <= 1}
          onclick={() => page--}
        >
          Previous
        </button>
        <span class="text-muted-foreground tabular-nums">
          Page {page} of {totalPages}
        </span>
        <button
          class="px-3 py-1.5 rounded-md border border-border bg-card text-sm
                 hover:bg-muted/50 transition-colors disabled:opacity-40 disabled:pointer-events-none"
          disabled={page >= totalPages}
          onclick={() => page++}
        >
          Next
        </button>
      </div>
    {/if}

    <Footer />
  </div>
</div>
