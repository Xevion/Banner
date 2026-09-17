<script lang="ts">
import { goto } from "$app/navigation";
import { client, type SearchResponse } from "$lib/api";
import { formatInstructorName, NameFormat } from "$lib/course";
import { dependOn, range } from "$lib/utils";
import type { PublicInstructorListItem, SearchOptionsResponse } from "$lib/bindings";
import Breadcrumb from "$lib/components/Breadcrumb.svelte";
import Footer from "$lib/components/Footer.svelte";
import InstructorCard from "$lib/components/InstructorCard.svelte";
import TermCombobox from "$lib/components/TermCombobox.svelte";
import { setCourseDetailContext } from "$lib/components/course-detail/context";
import { CourseTable } from "$lib/components/course-table";
import { untrack } from "svelte";

interface PageData {
  searchOptions: SearchOptionsResponse | null;
  searchResult: SearchResponse | null;
  searchError: string | null;
  subject: string;
  subjectDescription: string | null;
  term: string | null;
  instructors: PublicInstructorListItem[];
  instructorTotal: number;
}

let { data }: { data: PageData } = $props();

let selectedTerm = $state(untrack(() => data.term ?? ""));
let courses = $state(untrack(() => data.searchResult?.items ?? []));
let loading = $state(false);
let searchError = $state(untrack(() => data.searchError));

const terms = $derived(data.searchOptions?.terms ?? []);
setCourseDetailContext({ navigateToSection: null });

let columnVisibility = $state({ subject: false });

const sectionCount = $derived(courses.length);
const uniqueCourseCount = $derived.by(() => {
  // eslint-disable-next-line svelte/prefer-svelte-reactivity -- temporary set inside derivation
  const seen = new Set<string>();
  for (const c of courses) {
    seen.add(`${c.subject}-${c.courseNumber}`);
  }
  return seen.size;
});

async function onTermChange() {
  if (!selectedTerm) return;
  loading = true;
  const result = await client.searchCourses({
    term: selectedTerm,
    subject: [data.subject],
    perPage: 100,
  });
  if (result.isOk) {
    courses = result.value.items;
    searchError = null;
  } else {
    // Drop the stale rows, or they read as this term's sections.
    courses = [];
    searchError = result.error.message;
  }
  loading = false;

  void goto(`/subjects/${data.subject}?term=${encodeURIComponent(selectedTerm)}`, {
    replaceState: true,
    keepFocus: true,
  });
}

let termMounted = false;
$effect(() => {
  dependOn(selectedTerm);
  if (!termMounted) {
    termMounted = true;
    return;
  }
  void onTermChange();
});
</script>

<svelte:head>
  <title>{data.subject} &mdash; {data.subjectDescription ?? data.subject} | Banner</title>
</svelte:head>

<div class="min-h-screen flex flex-col items-center px-3 md:px-5 pb-5 pt-20">
  <div class="w-full max-w-6xl flex flex-col pt-2">
    <Breadcrumb
      items={[
        { label: "Home", href: "/" },
        { label: "Subjects", href: "/subjects" },
        { label: data.subject },
      ]}
    />

    <!-- Header -->
    <div class="mb-6">
      <h1 class="text-2xl font-bold">
        {data.subjectDescription ?? data.subject}
      </h1>
    </div>

    <!-- Term selector + stats -->
    <div class="flex flex-wrap items-center gap-3 mb-4">
      {#if terms.length > 1}
        <TermCombobox {terms} bind:value={selectedTerm} />
      {:else if terms[0]}
        <span class="text-sm text-muted-foreground">{terms[0].description}</span>
      {/if}

      <div class="flex items-center gap-3 text-sm text-muted-foreground">
        <span>{sectionCount} section{sectionCount !== 1 ? "s" : ""}</span>
        <span class="text-border">|</span>
        <span>{uniqueCourseCount} course{uniqueCourseCount !== 1 ? "s" : ""}</span>
      </div>
    </div>

    <!-- Instructors teaching this subject -->
    {#if data.instructors.length > 0}
      <section class="mb-6">
        <div class="flex items-baseline justify-between gap-3 mb-2">
          <h2 class="text-sm font-semibold">Instructors</h2>
          <a
            href="/instructors?subject={data.subject}&sort=score_desc"
            class="text-xs text-muted-foreground hover:text-foreground transition-colors"
          >
            View all {data.instructorTotal} &rarr;
          </a>
        </div>
        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
          {#each data.instructors as instructor (instructor.id)}
            <InstructorCard
              variant="panel"
              name={formatInstructorName(instructor.displayName, NameFormat.LastNameFirst)}
              slug={instructor.slug}
              rating={instructor.rating}
              rmp={instructor.rmp}
              bluebook={instructor.bluebook}
            />
          {/each}
        </div>
      </section>
    {/if}

    <!-- Course table -->
    {#if loading}
      <div class="rounded-lg border border-border overflow-hidden">
        <div class="animate-pulse">
          {#each range(8) as i (i)}
            <div class="flex gap-4 px-4 py-3 {i > 0 ? 'border-t border-border' : ''}">
              <div class="h-4 w-16 bg-muted rounded"></div>
              <div class="h-4 w-40 bg-muted rounded flex-1"></div>
              <div class="h-4 w-24 bg-muted rounded"></div>
              <div class="h-4 w-16 bg-muted rounded"></div>
            </div>
          {/each}
        </div>
      </div>
    {:else if searchError}
      <div
        class="flex flex-col items-center gap-2 text-center py-8 text-sm border border-destructive/40 bg-destructive/5 rounded-lg"
        role="alert"
      >
        <span class="text-destructive font-medium">Could not load sections</span>
        <span class="text-muted-foreground">{searchError}</span>
        <button
          onclick={() => void onTermChange()}
          class="mt-1 rounded-md border border-border px-3 py-1.5 text-sm font-medium
                 text-muted-foreground hover:bg-accent hover:text-accent-foreground cursor-pointer"
        >
          Try again
        </button>
      </div>
    {:else if courses.length > 0}
      <CourseTable
        {courses}
        loading={false}
        bind:columnVisibility
      />
    {:else}
      <div class="text-center py-8 text-muted-foreground text-sm border border-border rounded-lg">
        No sections found for this subject and term.
      </div>
    {/if}

    <Footer />
  </div>
</div>
