<script lang="ts">
import type { PublicInstructorListResponse, SearchOptionsResponse } from "$lib/bindings";
import { client } from "$lib/api";
import { ratingStyle } from "$lib/course";
import { formatNumber } from "$lib/utils";
import { themeStore } from "$lib/stores/theme.svelte";
import { goto } from "$app/navigation";
import { Star, Search, Mail } from "@lucide/svelte";
import Footer from "$lib/components/Footer.svelte";
import SubjectCombobox from "$lib/components/SubjectCombobox.svelte";
import { Select } from "bits-ui";
import { untrack } from "svelte";

interface PageData {
  instructors: PublicInstructorListResponse | null;
  searchOptions: SearchOptionsResponse | null;
  url: URL;
}

let { data }: { data: PageData } = $props();

let instructors = $state(untrack(() => data.instructors));
let search = $state(untrack(() => data.url.searchParams.get("search") ?? ""));
let selectedSubjects = $state<string[]>(
  untrack(() =>
    data.url.searchParams.get("subject") ? [data.url.searchParams.get("subject")!] : []
  )
);
let selectedSort = $state(untrack(() => data.url.searchParams.get("sort") ?? "name_asc"));
let loading = $state(false);

const subjects = $derived(data.searchOptions?.subjects ?? []);
const subjectMap = $derived(
  new Map(subjects.map((s: { code: string; description: string }) => [s.code, s.description]))
);
const totalPages = $derived(
  instructors ? Math.ceil(Number(instructors.total) / instructors.perPage) : 0
);

const sortItems = [
  { value: "name_asc", label: "Name A–Z" },
  { value: "name_desc", label: "Name Z–A" },
  { value: "rating_desc", label: "Highest Rated" },
];
const sortLabel = $derived(sortItems.find((s) => s.value === selectedSort)?.label ?? "Sort");

let searchTimeout: ReturnType<typeof setTimeout> | undefined;

function updateURL() {
  // eslint-disable-next-line svelte/prefer-svelte-reactivity -- non-reactive; used only for serializing to query string
  const params = new URLSearchParams();
  if (search) params.set("search", search);
  if (selectedSubjects.length === 1) params.set("subject", selectedSubjects[0]);
  if (selectedSort && selectedSort !== "name_asc") params.set("sort", selectedSort);
  if (instructors?.page && instructors.page > 1) params.set("page", String(instructors.page));
  const qs = params.toString();
  void goto(`/instructors${qs ? `?${qs}` : ""}`, { replaceState: true, keepFocus: true });
}

async function fetchInstructors(page = 1) {
  loading = true;
  const result = await client.getInstructors({
    search: search || undefined,
    subject: selectedSubjects[0] || undefined,
    sort: selectedSort,
    page,
  });
  if (result.isOk) {
    instructors = result.value;
  }
  loading = false;
  updateURL();
}

function onSearchInput() {
  clearTimeout(searchTimeout);
  searchTimeout = setTimeout(() => void fetchInstructors(), 300);
}

let mounted = false;
$effect(() => {
  void selectedSubjects;
  void selectedSort;
  if (!mounted) {
    mounted = true;
    return;
  }
  void fetchInstructors();
});

function goToPage(page: number) {
  void fetchInstructors(page);
}

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
          oninput={onSearchInput}
          class="w-full h-9 pl-9 pr-3 text-sm rounded-md border border-border bg-card
                 focus:outline-none focus:ring-2 focus:ring-ring"
        />
      </div>

      <SubjectCombobox {subjects} bind:value={selectedSubjects} />

      <Select.Root
        type="single"
        value={selectedSort}
        onValueChange={(v: string) => { if (v) selectedSort = v; }}
        items={sortItems}
      >
        <Select.Trigger
          class="inline-flex items-center justify-between gap-1.5 h-9 px-3
                 rounded-md border border-border bg-card text-sm text-muted-foreground
                 hover:bg-muted/50 transition-colors cursor-pointer select-none outline-none
                 focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background
                 w-36"
        >
          {sortLabel}
        </Select.Trigger>
        <Select.Portal>
          <Select.Content
            class="border border-border bg-card shadow-md outline-hidden z-50
                   min-w-36 select-none rounded-md p-1
                   data-[state=open]:animate-in data-[state=closed]:animate-out
                   data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0
                   data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95
                   data-[side=bottom]:slide-in-from-top-2"
            sideOffset={4}
          >
            <Select.Viewport class="p-0.5">
              {#each sortItems as item (item.value)}
                <Select.Item
                  class="rounded-sm outline-hidden flex h-8 w-full select-none items-center
                         px-2.5 text-sm cursor-pointer
                         data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground
                         data-[selected]:font-medium"
                  value={item.value}
                  label={item.label}
                >
                  {item.label}
                </Select.Item>
              {/each}
            </Select.Viewport>
          </Select.Content>
        </Select.Portal>
      </Select.Root>
    </div>

    <!-- Results count -->
    {#if instructors && !loading}
      <p class="text-xs text-muted-foreground mb-3">
        {formatNumber(Number(instructors.total))} instructor{Number(instructors.total) !== 1 ? "s" : ""} found
      </p>
    {/if}

    <!-- Card grid -->
    {#if loading && !instructors}
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
        class:opacity-50={loading}
      >
        {#if instructors}
          {#each instructors.instructors as instructor (instructor.id)}
            <a
              href="/instructors/{instructor.slug}"
              class="block rounded-lg border border-border bg-card p-4
                     hover:border-foreground/20 hover:shadow-sm transition-all"
            >
              <div class="flex items-start justify-between gap-2">
                <div class="min-w-0 flex-1">
                  <h2 class="font-semibold text-sm truncate">{instructor.displayName}</h2>
                  <div class="flex items-center gap-1 mt-0.5 text-xs text-muted-foreground">
                    <Mail class="size-3 shrink-0" />
                    <span class="truncate">{instructor.email}</span>
                  </div>
                </div>

                {#if instructor.avgRating != null && instructor.numRatings != null}
                  <div class="flex items-center gap-1 shrink-0">
                    <span
                      class="text-sm font-semibold inline-flex items-center gap-0.5"
                      style={ratingStyle(instructor.avgRating, themeStore.isDark)}
                    >
                      {instructor.avgRating.toFixed(1)}
                      <Star class="size-3 fill-current" />
                    </span>
                    <span class="text-[10px] text-muted-foreground">
                      ({formatNumber(instructor.numRatings)})
                    </span>
                  </div>
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
    {#if instructors?.instructors.length === 0 && !loading}
      <div class="text-center py-16 text-muted-foreground">
        <p class="text-sm">No instructors found matching your criteria.</p>
      </div>
    {/if}

    <!-- Pagination -->
    {#if instructors && totalPages > 1}
      <div class="flex justify-center items-center gap-2 mt-6 text-sm">
        <button
          class="px-3 py-1.5 rounded-md border border-border bg-card text-sm
                 hover:bg-muted/50 transition-colors disabled:opacity-40 disabled:pointer-events-none"
          disabled={instructors.page <= 1}
          onclick={() => goToPage(instructors!.page - 1)}
        >
          Previous
        </button>
        <span class="text-muted-foreground tabular-nums">
          Page {instructors.page} of {totalPages}
        </span>
        <button
          class="px-3 py-1.5 rounded-md border border-border bg-card text-sm
                 hover:bg-muted/50 transition-colors disabled:opacity-40 disabled:pointer-events-none"
          disabled={instructors.page >= totalPages}
          onclick={() => goToPage(instructors!.page + 1)}
        >
          Next
        </button>
      </div>
    {/if}

    <Footer />
  </div>
</div>
