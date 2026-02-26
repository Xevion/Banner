<script lang="ts">
import { goto } from "$app/navigation";
import { client } from "$lib/api";
import type { SearchOptionsResponse, SearchResponse } from "$lib/bindings";
import Breadcrumb from "$lib/components/Breadcrumb.svelte";
import Footer from "$lib/components/Footer.svelte";
import TermCombobox from "$lib/components/TermCombobox.svelte";
import { buildAttributeMap, setCourseDetailContext } from "$lib/components/course-detail/context";
import { CourseTable } from "$lib/components/course-table";
import { untrack } from "svelte";

interface PageData {
  searchOptions: SearchOptionsResponse | null;
  searchResult: SearchResponse | null;
  subject: string;
  subjectDescription: string | null;
  term: string | null;
}

let { data }: { data: PageData } = $props();

let selectedTerm = $state(untrack(() => data.term ?? ""));
let courses = $state(untrack(() => data.searchResult?.courses ?? []));
let loading = $state(false);

const terms = $derived(data.searchOptions?.terms ?? []);
const attributes = $derived(data.searchOptions?.reference.attributes ?? []);
const attributeMap = $derived(buildAttributeMap(attributes));

setCourseDetailContext({
  get attributeMap() {
    return attributeMap;
  },
  navigateToSection: null,
});

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
    limit: 100,
  });
  if (result.isOk) {
    courses = result.value.courses;
  }
  loading = false;

  void goto(`/subjects/${data.subject}?term=${encodeURIComponent(selectedTerm)}`, {
    replaceState: true,
    keepFocus: true,
  });
}

let termMounted = false;
$effect(() => {
  void selectedTerm;
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
      {:else if terms.length === 1}
        <span class="text-sm text-muted-foreground">{terms[0].description}</span>
      {/if}

      <div class="flex items-center gap-3 text-sm text-muted-foreground">
        <span>{sectionCount} section{sectionCount !== 1 ? "s" : ""}</span>
        <span class="text-border">|</span>
        <span>{uniqueCourseCount} course{uniqueCourseCount !== 1 ? "s" : ""}</span>
      </div>
    </div>

    <!-- Course table -->
    {#if loading}
      <div class="rounded-lg border border-border overflow-hidden">
        <div class="animate-pulse">
          {#each Array(8) as _, i (i)}
            <div class="flex gap-4 px-4 py-3 {i > 0 ? 'border-t border-border' : ''}">
              <div class="h-4 w-16 bg-muted rounded"></div>
              <div class="h-4 w-40 bg-muted rounded flex-1"></div>
              <div class="h-4 w-24 bg-muted rounded"></div>
              <div class="h-4 w-16 bg-muted rounded"></div>
            </div>
          {/each}
        </div>
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
