<script lang="ts">
import { goto } from "$app/navigation";
import { client } from "$lib/api";
import type { PublicInstructorListResponse, SearchOptionsResponse } from "$lib/bindings";
import { useQuery } from "$lib/composables";
import Footer from "$lib/components/Footer.svelte";
import SubjectCombobox from "$lib/components/SubjectCombobox.svelte";
import SortSelect from "$lib/components/SortSelect.svelte";
import type { SortOption } from "$lib/components/SortSelect.svelte";
import ScoreBadge from "$lib/components/score/ScoreBadge.svelte";
import { formatNumber } from "$lib/utils";
import { ExternalLink, Mail, Search } from "@lucide/svelte";
import { untrack } from "svelte";

interface PageData {
  instructors: PublicInstructorListResponse | null;
  searchOptions: SearchOptionsResponse | null;
  url: URL;
}

let { data }: { data: PageData } = $props();

let search = $state(untrack(() => data.url.searchParams.get("search") ?? ""));
let selectedSubjects = $state<string[]>(
  untrack(() =>
    data.url.searchParams.get("subject") ? [data.url.searchParams.get("subject")!] : []
  )
);
let selectedSort = $state(untrack(() => data.url.searchParams.get("sort") ?? "name_asc"));
let page = $state(untrack(() => Number(data.url.searchParams.get("page")) || 1));

const subjects = $derived(data.searchOptions?.subjects ?? []);
const subjectMap = $derived(
  new Map(subjects.map((s: { code: string; description: string }) => [s.code, s.description]))
);

const sortOptions: SortOption[] = [
  { value: "name", label: "Alphabetical", defaultDirection: "asc" },
  { value: "rating", label: "Rating", defaultDirection: "desc" },
];

const query = useQuery({
  fetcher: () =>
    client.getInstructors({
      search: search || undefined,
      subject: selectedSubjects[0] || undefined,
      sort: selectedSort,
      page,
    }),
  deps: () => [search, selectedSubjects[0], selectedSort, page],
  debounce: 300,
  initial: untrack(() => data.instructors),
});

const totalPages = $derived(
  query.data ? Math.ceil(Number(query.data.total) / query.data.perPage) : 0
);

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
  // eslint-disable-next-line svelte/prefer-svelte-reactivity
  const params = new URLSearchParams();
  if (search) params.set("search", search);
  if (selectedSubjects.length === 1) params.set("subject", selectedSubjects[0]);
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
        {formatNumber(Number(query.data.total))} instructor{Number(query.data.total) !== 1 ? "s" : ""} found
      </p>
    {/if}

    <!-- Card grid -->
    {#if query.isLoading && !query.data}
      <!-- Skeleton grid for initial load -->
      <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {#each Array(12) as _, i (i)}
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
          {#each query.data.instructors as instructor (instructor.id)}
            <a
              href="/instructors/{instructor.slug}"
              class="block rounded-lg border border-border bg-card p-4
                     hover:border-foreground/20 hover:shadow-sm transition-all"
            >
              <div class="flex items-start justify-between gap-2">
                <div class="min-w-0 flex-1">
                  <h2 class="font-semibold text-sm truncate">{instructor.displayName}</h2>
                  {#if instructor.email}
                    <div class="flex items-center gap-1 mt-0.5 text-xs text-muted-foreground">
                      <Mail class="size-3 shrink-0" />
                      <span class="truncate">{instructor.email}</span>
                    </div>
                  {/if}
                </div>

                {#if instructor.composite != null}
                  <div class="shrink-0">
                    <ScoreBadge
                      score={instructor.composite.score}
                      source={instructor.bluebook != null && instructor.rmp?.avgRating == null ? "bluebook" : "composite"}
                      size="sm"
                    />
                  </div>
                {:else if instructor.rmp != null}
                  <span class="text-muted-foreground shrink-0" title="View on RateMyProfessors">
                    <ExternalLink class="size-3.5" />
                  </span>
                {/if}
              </div>

              {#if instructor.subjects.length > 0}
                <div class="flex flex-wrap gap-1 mt-2.5">
                  {#each instructor.subjects.slice(0, 4) as subject (subject)}
                    <span
                      class="inline-block px-1.5 py-0.5 text-[10px] font-medium rounded
                             bg-muted text-muted-foreground truncate max-w-32"
                    >
                      {resolveSubject(subject)}
                    </span>
                  {/each}
                  {#if instructor.subjects.length > 4}
                    <span class="text-[10px] text-muted-foreground self-center">
                      +{instructor.subjects.length - 4}
                    </span>
                  {/if}
                </div>
              {/if}
            </a>
          {/each}
        {/if}
      </div>
    {/if}

    <!-- Empty state -->
    {#if query.data?.instructors.length === 0 && !query.isLoading}
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
