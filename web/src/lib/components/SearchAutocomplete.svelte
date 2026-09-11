<script lang="ts">
import type { Subject } from "$lib/api";
import { client } from "$lib/api";
import type { SuggestResponse } from "$lib/bindings";
import { useSuggestions } from "$lib/composables/useSuggestions.svelte";
import { populateInstructorCache } from "$lib/filters";
import { getFiltersContext } from "$lib/stores/search-filters.svelte";
import type { Suggestion } from "$lib/suggestions";
import { createSubjectSearch, mergeSuggestions, suggestionId } from "$lib/suggestions";
import { BookOpen, GraduationCap, Loader2, Search, TriangleAlert, User } from "@lucide/svelte";
import { Command, Popover } from "bits-ui";
import { fly } from "svelte/transition";

let {
  subjects,
  selectedTerm,
}: {
  subjects: Subject[];
  selectedTerm: string;
} = $props();

const filters = getFiltersContext();

let anchorEl = $state<HTMLDivElement>(null!);

const query = useSuggestions<SuggestResponse>({
  fetcher: (q) => client.suggest(selectedTerm, q),
  empty: { courses: [], instructors: [] },
});

const subjectSearch = $derived(createSubjectSearch(subjects));
const selectedInstructors = $derived(new Set(filters.instructor));

const suggestions = $derived(
  mergeSuggestions(query.text, subjectSearch, query.data, selectedInstructors)
);

/** A search that came back with nothing, as opposed to one still running. */
const isEmpty = $derived(
  query.settled && !query.loading && suggestions.length === 0 && !query.error
);

function addSubject(code: string) {
  if (!filters.subject.includes(code)) {
    filters.subject = [...filters.subject, code];
  }
}

function apply(suggestion: Suggestion) {
  switch (suggestion.kind) {
    case "subject":
      addSubject(suggestion.subject.code);
      break;
    case "course":
      addSubject(suggestion.course.subject);
      filters.query = suggestion.course.title;
      break;
    case "instructor": {
      const { slug, displayName } = suggestion.instructor;
      populateInstructorCache({ [slug]: displayName });
      if (!filters.instructor.includes(slug)) {
        filters.instructor = [...filters.instructor, slug];
      }
      break;
    }
  }
  query.reset();
}

function handleKeydown(e: KeyboardEvent) {
  // With no list showing there is nothing to choose, so Enter searches for
  // whatever was typed rather than waiting on a suggestion.
  if (e.key === "Enter" && !query.open) {
    e.preventDefault();
    filters.query = query.trimmed || null;
  }

  if (e.key === "Escape") query.open = false;
}

const listId = "search-autocomplete-list";
</script>

<Command.Root shouldFilter={false} class="relative flex-1 min-w-0 md:min-w-[200px]">
  <Popover.Root bind:open={query.open}>
    <div class="relative" bind:this={anchorEl}>
      {#if query.loading}
        <Loader2
          class="absolute left-3 top-1/2 -translate-y-1/2 size-3.5 text-muted-foreground pointer-events-none animate-spin"
        />
      {:else}
        <Search
          class="absolute left-3 top-1/2 -translate-y-1/2 size-3.5 text-muted-foreground pointer-events-none"
        />
      {/if}
      <Command.Input
        bind:value={() => query.text, (v: string) => query.setQuery(v)}
        onkeydown={handleKeydown}
        onfocus={() => {
          if (query.isLongEnough) query.open = true;
        }}
        placeholder="Search courses, subjects, or instructors..."
        aria-label="Search courses, subjects, or instructors"
        aria-expanded={query.open}
        aria-haspopup="listbox"
        aria-controls={listId}
        role="combobox"
        autocomplete="off"
        autocorrect="off"
        spellcheck={false}
        class="h-9 w-full border border-border bg-card text-foreground rounded-md pl-9 pr-3 text-sm
               focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background
               transition-colors"
      />
    </div>
    <Popover.Content
      class="z-50"
      customAnchor={anchorEl}
      sideOffset={4}
      align="start"
      trapFocus={false}
      onOpenAutoFocus={(e) => e.preventDefault()}
      onCloseAutoFocus={(e) => e.preventDefault()}
      forceMount
    >
      {#snippet child({ wrapperProps, props, open: isOpen })}
        {#if isOpen}
          <div {...wrapperProps}>
            <div {...props} transition:fly={{ duration: 150, y: -4 }}>
              <div
                id={listId}
                class="border border-border bg-card shadow-md rounded-md
                       w-[var(--bits-popover-anchor-width)] min-w-[280px] max-w-[480px]
                       max-h-72 overflow-y-auto scrollbar-none p-1"
              >
                <!--
                  These sit outside the list rather than in it: a listbox is
                  required to hold options, and one holding only a sentence is
                  invalid markup that a screen reader has to guess at.
                -->
                {#if query.error}
                  <div role="alert" class="flex items-center gap-1.5 px-2 py-2 text-sm text-destructive">
                    <TriangleAlert class="size-3.5 shrink-0" />
                    {query.error}
                  </div>
                {:else if query.loading && suggestions.length === 0}
                  <div role="status" class="flex items-center gap-1.5 px-2 py-2 text-sm text-muted-foreground">
                    <Loader2 class="size-3.5 animate-spin shrink-0" />
                    Searching...
                  </div>
                {:else if isEmpty}
                  <div role="status" class="px-2 py-2 text-sm text-muted-foreground">
                    No results found.
                  </div>
                {/if}

                <Command.List>
                {#each suggestions as item (suggestionId(item))}
                  <Command.Item
                    class="rounded-sm outline-hidden flex h-8 w-full select-none items-center gap-2 px-2 text-sm whitespace-nowrap
                           data-[selected]:bg-accent data-[selected]:text-accent-foreground cursor-pointer"
                    value={suggestionId(item)}
                    onSelect={() => apply(item)}
                  >
                    {#if item.kind === "subject"}
                      <BookOpen class="size-3.5 shrink-0 text-muted-foreground" />
                      <span
                        class="inline-flex items-center justify-center rounded bg-muted px-1 py-0.5
                               text-xs font-mono text-muted-foreground w-10 shrink-0 text-center"
                        >{item.subject.code}</span
                      >
                      <span class="flex-1 truncate">{item.subject.description}</span>
                    {:else if item.kind === "course"}
                      <GraduationCap class="size-3.5 shrink-0 text-muted-foreground" />
                      <span
                        class="inline-flex items-center justify-center rounded bg-muted px-1 py-0.5
                               text-xs font-mono text-muted-foreground shrink-0 text-center"
                        >{item.course.subject} {item.course.courseNumber}</span
                      >
                      <span class="flex-1 truncate">{item.course.title}</span>
                      <span class="text-xs text-muted-foreground shrink-0"
                        >{item.course.sectionCount}
                        {item.course.sectionCount === 1 ? "section" : "sections"}</span
                      >
                    {:else if item.kind === "instructor"}
                      <User class="size-3.5 shrink-0 text-muted-foreground" />
                      <span class="flex-1 truncate">{item.instructor.displayName}</span>
                      <span class="text-xs text-muted-foreground shrink-0"
                        >{item.instructor.sectionCount}
                        {item.instructor.sectionCount === 1 ? "section" : "sections"}</span
                      >
                    {/if}
                  </Command.Item>
                {/each}
                </Command.List>

                {#if query.loading && suggestions.length > 0}
                  <div role="status" class="px-2 py-1.5 text-xs text-muted-foreground italic">
                    Updating...
                  </div>
                {/if}
              </div>
            </div>
          </div>
        {/if}
      {/snippet}
    </Popover.Content>
  </Popover.Root>
</Command.Root>
