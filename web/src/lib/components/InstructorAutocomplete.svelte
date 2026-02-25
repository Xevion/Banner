<script lang="ts">
import { client } from "$lib/api";
import type { InstructorSuggestion } from "$lib/bindings";
import FilterChip from "$lib/components/FilterChip.svelte";
import { instructorDisplayName, populateInstructorCache } from "$lib/filters";
import { getFiltersContext } from "$lib/stores/search-filters.svelte";
import { Loader2, Search } from "@lucide/svelte";
import { Command } from "bits-ui";

let { selectedTerm }: { selectedTerm: string } = $props();

const filters = getFiltersContext();

let inputValue = $state("");
let open = $state(false);
let loading = $state(false);
let serverResults = $state<InstructorSuggestion[]>([]);
let debounceTimer: ReturnType<typeof setTimeout> | undefined;
let fetchId = 0;

$effect(() => {
  return () => clearTimeout(debounceTimer);
});

const selectedSlugs = $derived(new Set(filters.instructor));

const filteredResults = $derived(serverResults.filter((i) => !selectedSlugs.has(i.slug)));

async function fetchSuggestions() {
  const currentFetchId = ++fetchId;
  const q = inputValue.trim();
  if (q.length < 2) {
    serverResults = [];
    loading = false;
    return;
  }
  const result = await client.suggestInstructors(q, selectedTerm);
  if (currentFetchId !== fetchId) return;
  result.match({
    Ok: (data) => {
      serverResults = data;
    },
    Err: () => {
      serverResults = [];
    },
  });
  loading = false;
}

function handleInput(value: string) {
  inputValue = value;
  clearTimeout(debounceTimer);
  fetchId++;

  if (value.trim().length < 2) {
    serverResults = [];
    loading = false;
    open = false;
    return;
  }

  open = true;
  loading = true;
  debounceTimer = setTimeout(() => void fetchSuggestions(), 250);
}

function select(instructor: InstructorSuggestion) {
  populateInstructorCache({ [instructor.slug]: instructor.displayName });
  if (!filters.instructor.includes(instructor.slug)) {
    filters.instructor = [...filters.instructor, instructor.slug];
  }
  inputValue = "";
  serverResults = [];
  open = false;
}

function remove(slug: string) {
  filters.instructor = filters.instructor.filter((i) => i !== slug);
}
</script>

<div class="flex flex-col gap-1.5">
  <span class="text-xs font-medium text-muted-foreground select-none">Instructor</span>

  {#if filters.instructor.length > 0}
    <div class="flex flex-wrap gap-1 mb-0.5">
      {#each filters.instructor as slug (slug)}
        <FilterChip label={instructorDisplayName(slug)} onRemove={() => remove(slug)} />
      {/each}
    </div>
  {/if}

  <Command.Root shouldFilter={false} class="relative">
    <div class="relative">
      {#if loading}
        <Loader2
          class="absolute left-2 top-1/2 -translate-y-1/2 size-3.5 text-muted-foreground animate-spin pointer-events-none"
        />
      {:else}
        <Search
          class="absolute left-2 top-1/2 -translate-y-1/2 size-3.5 text-muted-foreground pointer-events-none"
        />
      {/if}
      <Command.Input
        bind:value={inputValue}
        oninput={(e: Event & { currentTarget: HTMLInputElement }) =>
          handleInput(e.currentTarget.value)}
        onblur={() =>
          setTimeout(() => {
            open = false;
          }, 150)}
        placeholder="Search instructors..."
        autocomplete="off"
        class="h-8 w-full border border-border bg-card text-foreground rounded-md pl-8 pr-2 text-sm
               focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background"
      />
    </div>
    {#if open}
      <Command.List
        class="absolute top-full left-0 right-0 z-10 mt-1 border border-border bg-card shadow-md rounded-md
               max-h-40 overflow-y-auto scrollbar-none p-1"
      >
        {#if loading && filteredResults.length === 0}
          <div class="flex items-center gap-1.5 px-2 py-2 text-xs text-muted-foreground">
            <Loader2 class="size-3 animate-spin shrink-0" />
            Searching...
          </div>
        {:else if filteredResults.length === 0}
          <div class="px-2 py-2 text-xs text-muted-foreground">No results found.</div>
        {:else}
          {#each filteredResults as instructor (instructor.id)}
            <Command.Item
              class="rounded-sm outline-hidden flex h-8 w-full select-none items-center gap-2 px-2 text-sm
                     data-[selected]:bg-accent data-[selected]:text-accent-foreground cursor-pointer"
              value={instructor.slug}
              onSelect={() => select(instructor)}
            >
              <span class="flex-1 truncate">{instructor.displayName}</span>
              <span class="text-xs text-muted-foreground shrink-0"
                >{instructor.sectionCount}
                {instructor.sectionCount === 1 ? "section" : "sections"}</span
              >
            </Command.Item>
          {/each}
        {/if}
      </Command.List>
    {/if}
  </Command.Root>
</div>
