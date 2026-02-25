<script lang="ts">
import type { Subject } from "$lib/api";
import { client } from "$lib/api";
import type { CourseSuggestion, InstructorSuggestion, SuggestResponse } from "$lib/bindings";
import { getFiltersContext } from "$lib/stores/search-filters.svelte";
import { BookOpen, GraduationCap, Loader2, Search, TriangleAlert, User } from "@lucide/svelte";
import createFuzzySearch from "@nozbe/microfuzz";
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

let searchValue = $state("");
let open = $state(false);
let triggerRef = $state<HTMLDivElement>(null!);
let serverResults = $state<SuggestResponse>({ courses: [], instructors: [] });
let loading = $state(false);
let error = $state<string | null>(null);
/** Whether we've received at least one server response for the current query */
let hasServerResponse = $state(false);
let debounceTimer: ReturnType<typeof setTimeout> | undefined;
/** Monotonic counter to discard stale fetch responses */
let fetchId = 0;

// Clean up debounce timer on component destroy
$effect(() => {
  return () => clearTimeout(debounceTimer);
});

const fuzzySearch = $derived(
  createFuzzySearch(subjects, {
    getText: (item: Subject) => [item.code, item.description],
  })
);

type ScoredSuggestion =
  | { kind: "subject"; subject: Subject; score: number }
  | { kind: "course"; course: CourseSuggestion; score: number }
  | { kind: "instructor"; instructor: InstructorSuggestion; score: number };

const mergedSuggestions = $derived.by((): ScoredSuggestion[] => {
  const q = searchValue.trim();
  if (q.length < 2) return [];

  const items: ScoredSuggestion[] = [];

  // Client-side fuzzy subject matches (microfuzz scores are 0..1 where higher is better)
  for (const r of fuzzySearch(q).slice(0, 5)) {
    // microfuzz doesn't expose a numeric score directly; use match presence as 0.5 baseline
    items.push({ kind: "subject", subject: r.item, score: 0.5 });
  }

  // Server-side results with trigram similarity scores
  for (const c of serverResults.courses) {
    items.push({ kind: "course", course: c, score: c.score });
  }
  for (const i of serverResults.instructors) {
    items.push({ kind: "instructor", instructor: i, score: i.score });
  }

  // Sort all items together by score descending
  items.sort((a, b) => b.score - a.score);

  return items;
});

const hasSuggestions = $derived(mergedSuggestions.length > 0);

/** True when the server has responded with zero results and local fuzzy also found nothing */
const isEmpty = $derived(hasServerResponse && !loading && !hasSuggestions && !error);

async function fetchSuggestions() {
  const currentFetchId = ++fetchId;
  const q = searchValue.trim();
  if (q.length < 2) {
    serverResults = { courses: [], instructors: [] };
    loading = false;
    error = null;
    hasServerResponse = false;
    return;
  }
  const result = await client.suggest(selectedTerm, q);
  // Discard stale responses -- a newer request has been issued
  if (currentFetchId !== fetchId) return;
  result.match({
    Ok: (data) => {
      serverResults = data;
      error = null;
    },
    Err: (e) => {
      serverResults = { courses: [], instructors: [] };
      error = e.message ?? "Failed to fetch suggestions";
    },
  });
  loading = false;
  hasServerResponse = true;
}

function handleInput(value: string) {
  searchValue = value;
  clearTimeout(debounceTimer);
  // Invalidate any in-flight fetch
  fetchId++;

  if (value.trim().length < 2) {
    serverResults = { courses: [], instructors: [] };
    loading = false;
    error = null;
    hasServerResponse = false;
    open = false;
    return;
  }

  open = true;
  loading = true;
  error = null;
  hasServerResponse = false;
  debounceTimer = setTimeout(() => void fetchSuggestions(), 250);
}

function handleSelect(value: string) {
  const [type, ...rest] = value.split(":");
  switch (type) {
    case "subject": {
      const code = rest[0];
      if (!filters.subject.includes(code)) {
        filters.subject = [...filters.subject, code];
      }
      break;
    }
    case "course": {
      const [subject, , ...titleParts] = rest;
      if (!filters.subject.includes(subject)) {
        filters.subject = [...filters.subject, subject];
      }
      filters.query = titleParts.join(":");
      break;
    }
    case "instructor": {
      const displayName = rest.slice(1).join(":");
      filters.instructor = displayName;
      break;
    }
  }

  searchValue = "";
  serverResults = { courses: [], instructors: [] };
  hasServerResponse = false;
  open = false;
}

function handleKeydown(e: KeyboardEvent) {
  // Enter with no open popover submits as free-text search
  if (e.key === "Enter" && !open) {
    e.preventDefault();
    filters.query = searchValue.trim() || null;
  }

  // Escape closes the popover but keeps text
  if (e.key === "Escape") {
    open = false;
  }
}

const popoverListId = "search-autocomplete-list";
</script>

<Command.Root
  shouldFilter={false}
  class="relative flex-1 min-w-0 md:min-w-[200px]"
>
  <Popover.Root bind:open>
    <Popover.Trigger bind:ref={triggerRef}>
      {#snippet child({ props })}
        {@const { onkeydown: _a, onclick: _b, ...triggerProps } = props as Record<string, unknown>}
        <div
          {...triggerProps}
          onclick={() => {
            if (searchValue.trim().length >= 2) open = !open;
          }}
          class="relative"
        >
          {#if loading}
            <Loader2
              class="absolute left-3 top-1/2 -translate-y-1/2 size-3.5 text-muted-foreground pointer-events-none animate-spin"
            />
          {:else}
            <Search
              class="absolute left-3 top-1/2 -translate-y-1/2 size-3.5 text-muted-foreground pointer-events-none"
            />
          {/if}
          <Command.Input
            bind:value={searchValue}
            oninput={(e: Event & { currentTarget: HTMLInputElement }) => handleInput(e.currentTarget.value)}
            onkeydown={handleKeydown}
            onfocus={() => {
              if (searchValue.trim().length >= 2) open = true;
            }}
            onblur={() => {
              // Delay so that clicking a suggestion item fires onSelect before we close
              setTimeout(() => { open = false; }, 150);
            }}
            placeholder="Search courses, subjects, or instructors..."
            aria-label="Search courses, subjects, or instructors"
            aria-expanded={open}
            aria-haspopup="listbox"
            aria-controls={popoverListId}
            role="combobox"
            autocomplete="off"
            autocorrect="off"
            spellcheck={false}
            class="h-9 w-full border border-border bg-card text-foreground rounded-md pl-9 pr-3 text-sm
                   focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background
                   transition-colors"
          />
        </div>
      {/snippet}
    </Popover.Trigger>
    <Popover.Content
        sideOffset={4}
        align="start"
        onOpenAutoFocus={(e) => e.preventDefault()}
        onCloseAutoFocus={(e) => e.preventDefault()}
        onInteractOutside={(e) => {
          if (triggerRef?.contains(e.target as Node)) e.preventDefault();
        }}
        forceMount
      >
        {#snippet child({ wrapperProps, props, open: isOpen })}
          {#if isOpen}
            <div {...wrapperProps} class="z-50">
              <div
                {...props}
                transition:fly={{ duration: 150, y: -4 }}
              >
                <Command.List
                  id={popoverListId}
                  class="border border-border bg-card shadow-md rounded-md
                         w-[var(--bits-popover-anchor-width)] min-w-[280px] max-w-[480px]
                         max-h-72 overflow-y-auto scrollbar-none p-1"
                >
                  {#if error}
                    <div class="flex items-center gap-1.5 px-2 py-2 text-sm text-destructive">
                      <TriangleAlert class="size-3.5 shrink-0" />
                      {error}
                    </div>
                  {:else if loading && !hasSuggestions}
                    <div class="flex items-center gap-1.5 px-2 py-2 text-sm text-muted-foreground">
                      <Loader2 class="size-3.5 animate-spin shrink-0" />
                      Searching...
                    </div>
                  {:else if isEmpty}
                    <div class="px-2 py-2 text-sm text-muted-foreground">
                      No results found.
                    </div>
                  {/if}

                  {#each mergedSuggestions as item (item.kind === "subject" ? `s:${item.subject.code}` : item.kind === "course" ? `c:${item.course.subject}:${item.course.courseNumber}:${item.course.title}` : `i:${item.instructor.id}`)}
                    {#if item.kind === "subject"}
                      {@const s = item.subject}
                      <Command.Item
                        class="rounded-sm outline-hidden flex h-8 w-full select-none items-center gap-2 px-2 text-sm whitespace-nowrap
                               data-[selected]:bg-accent data-[selected]:text-accent-foreground cursor-pointer"
                        value="subject:{s.code}"
                        keywords={[s.code, s.description]}
                        onSelect={() => handleSelect(`subject:${s.code}`)}
                      >
                        <BookOpen class="size-3.5 shrink-0 text-muted-foreground" />
                        <span
                          class="inline-flex items-center justify-center rounded bg-muted px-1 py-0.5
                                 text-xs font-mono text-muted-foreground w-10 shrink-0 text-center"
                          >{s.code}</span
                        >
                        <span class="flex-1 truncate">{s.description}</span>
                      </Command.Item>
                    {:else if item.kind === "course"}
                      {@const c = item.course}
                      <Command.Item
                        class="rounded-sm outline-hidden flex h-8 w-full select-none items-center gap-2 px-2 text-sm whitespace-nowrap
                               data-[selected]:bg-accent data-[selected]:text-accent-foreground cursor-pointer"
                        value="course:{c.subject}:{c.courseNumber}:{c.title}"
                        keywords={[c.subject, c.courseNumber, c.title]}
                        onSelect={() => handleSelect(`course:${c.subject}:${c.courseNumber}:${c.title}`)}
                      >
                        <GraduationCap class="size-3.5 shrink-0 text-muted-foreground" />
                        <span
                          class="inline-flex items-center justify-center rounded bg-muted px-1 py-0.5
                                 text-xs font-mono text-muted-foreground shrink-0 text-center"
                          >{c.subject} {c.courseNumber}</span
                        >
                        <span class="flex-1 truncate">{c.title}</span>
                        <span class="text-xs text-muted-foreground shrink-0"
                          >{c.sectionCount} {c.sectionCount === 1 ? 'section' : 'sections'}</span
                        >
                      </Command.Item>
                    {:else if item.kind === "instructor"}
                      {@const i = item.instructor}
                      <Command.Item
                        class="rounded-sm outline-hidden flex h-8 w-full select-none items-center gap-2 px-2 text-sm whitespace-nowrap
                               data-[selected]:bg-accent data-[selected]:text-accent-foreground cursor-pointer"
                        value="instructor:{i.id}:{i.displayName}"
                        keywords={[i.displayName]}
                        onSelect={() => handleSelect(`instructor:${i.id}:${i.displayName}`)}
                      >
                        <User class="size-3.5 shrink-0 text-muted-foreground" />
                        <span class="flex-1 truncate">{i.displayName}</span>
                        <span class="text-xs text-muted-foreground shrink-0"
                          >{i.sectionCount} {i.sectionCount === 1 ? 'section' : 'sections'}</span
                        >
                      </Command.Item>
                    {/if}
                  {/each}

                  {#if loading && hasSuggestions}
                    <div class="px-2 py-1.5 text-xs text-muted-foreground italic">
                      Updating...
                    </div>
                  {/if}
                </Command.List>
              </div>
            </div>
          {/if}
        {/snippet}
      </Popover.Content>
  </Popover.Root>
</Command.Root>
