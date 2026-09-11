<script lang="ts">
import { client } from "$lib/api";
import type { InstructorSuggestion } from "$lib/bindings";
import FilterChip from "$lib/components/FilterChip.svelte";
import { useSuggestions } from "$lib/composables/useSuggestions.svelte";
import { instructorDisplayName, populateInstructorCache } from "$lib/filters";
import { getFiltersContext } from "$lib/stores/search-filters.svelte";
import { Loader2, Search, TriangleAlert } from "@lucide/svelte";
import { Command } from "bits-ui";

let { selectedTerm }: { selectedTerm: string } = $props();

const filters = getFiltersContext();

const query = useSuggestions<InstructorSuggestion[]>({
  fetcher: (q) => client.suggestInstructors(q, selectedTerm),
  empty: [],
});

const selectedSlugs = $derived(new Set(filters.instructor));

const results = $derived(query.data.filter((i) => !selectedSlugs.has(i.slug)));

function select(instructor: InstructorSuggestion) {
  populateInstructorCache({ [instructor.slug]: instructor.displayName });
  if (!filters.instructor.includes(instructor.slug)) {
    filters.instructor = [...filters.instructor, instructor.slug];
  }
  query.reset();
}

function remove(slug: string) {
  filters.instructor = filters.instructor.filter((i) => i !== slug);
}

const listId = "instructor-autocomplete-list";
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

  <Command.Root
    shouldFilter={false}
    class="relative"
    onfocusout={(e: FocusEvent & { currentTarget: HTMLElement }) => {
      // Replaces a 150ms close timer that fired even when focus came straight
      // back, and that only worked because clicks happened to beat it.
      if (!e.currentTarget.contains(e.relatedTarget as Node)) query.open = false;
    }}
  >
    <div class="relative">
      {#if query.loading}
        <Loader2
          class="absolute left-2 top-1/2 -translate-y-1/2 size-3.5 text-muted-foreground animate-spin pointer-events-none"
        />
      {:else}
        <Search
          class="absolute left-2 top-1/2 -translate-y-1/2 size-3.5 text-muted-foreground pointer-events-none"
        />
      {/if}
      <Command.Input
        bind:value={() => query.text, (v: string) => query.setQuery(v)}
        onfocus={() => {
          if (query.isLongEnough) query.open = true;
        }}
        onkeydown={(e: KeyboardEvent) => {
          // Dismiss the suggestions first and keep the surrounding popover open;
          // a second Escape, with no list showing, closes that instead.
          if (e.key === "Escape" && query.open) {
            e.stopPropagation();
            query.open = false;
          }
        }}
        placeholder="Search instructors..."
        aria-label="Search instructors"
        aria-expanded={query.open}
        aria-haspopup="listbox"
        aria-controls={listId}
        role="combobox"
        autocomplete="off"
        class="h-8 w-full border border-border bg-card text-foreground rounded-md pl-8 pr-2 text-sm
               focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background"
      />
    </div>
    {#if query.open}
      <div
        id={listId}
        class="absolute top-full left-0 right-0 z-10 mt-1 border border-border bg-card shadow-md rounded-md
               max-h-40 overflow-y-auto scrollbar-none p-1"
      >
        <!--
          Outside the list on purpose: a listbox holding a sentence instead of
          options is invalid, and a screen reader announces it as an empty list.
        -->
        {#if query.error}
          <div role="alert" class="flex items-center gap-1.5 px-2 py-2 text-xs text-destructive">
            <TriangleAlert class="size-3 shrink-0" />
            {query.error}
          </div>
        {:else if query.loading && results.length === 0}
          <div role="status" class="flex items-center gap-1.5 px-2 py-2 text-xs text-muted-foreground">
            <Loader2 class="size-3 animate-spin shrink-0" />
            Searching...
          </div>
        {:else if results.length === 0}
          <div role="status" class="px-2 py-2 text-xs text-muted-foreground">No results found.</div>
        {:else}
          <!-- Keeps the input focused: a blur here would close the list mid-click. -->
          <Command.List onmousedown={(e: MouseEvent) => e.preventDefault()}>
          {#each results as instructor (instructor.id)}
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
          </Command.List>
        {/if}
      </div>
    {/if}
  </Command.Root>
</div>
