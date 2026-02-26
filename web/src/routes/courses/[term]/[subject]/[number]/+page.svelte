<script lang="ts">
import type { CourseResponse, SearchOptionsResponse } from "$lib/bindings";
import Footer from "$lib/components/Footer.svelte";
import { buildAttributeMap, setCourseDetailContext } from "$lib/components/course-detail/context";
import { CourseTable } from "$lib/components/course-table";
import ScorePopover from "$lib/components/score/ScorePopover.svelte";
import { formatCreditHours, formatInstructorName } from "$lib/course";
import { getAttributeLabel, getInstructionalMethodLabel } from "$lib/labels";
import Breadcrumb from "$lib/components/Breadcrumb.svelte";
import { untrack } from "svelte";

interface PageData {
  sections: CourseResponse[];
  searchOptions: SearchOptionsResponse | null;
  term: string;
  subject: string;
  courseNumber: string;
}

let { data }: { data: PageData } = $props();

const sections = untrack(() => data.sections);
const representative = sections[0];

const attributes = $derived(data.searchOptions?.reference.attributes ?? []);
const attributeMap = $derived(buildAttributeMap(attributes));

const termDescription = $derived(
  data.searchOptions?.terms.find((t) => t.slug === data.term)?.description ?? data.term
);

setCourseDetailContext({
  get attributeMap() {
    return attributeMap;
  },
  navigateToSection: null,
});

let columnVisibility = $state({ course_code: false });

// Collect unique instructors across all sections
const uniqueInstructors = $derived.by(() => {
  // eslint-disable-next-line svelte/prefer-svelte-reactivity -- temporary set inside derivation
  const seen = new Set<number>();
  const result: CourseResponse["instructors"] = [];
  for (const section of sections) {
    for (const inst of section.instructors) {
      if (!seen.has(inst.instructorId)) {
        seen.add(inst.instructorId);
        result.push(inst);
      }
    }
  }
  return result;
});
</script>

<svelte:head>
  <title>{data.subject} {data.courseNumber} &mdash; {representative.title} | Banner</title>
</svelte:head>

<div class="min-h-screen flex flex-col items-center px-3 md:px-5 pb-5 pt-20">
  <div class="w-full max-w-6xl flex flex-col pt-2">
    <Breadcrumb
      items={[
        { label: "Home", href: "/" },
        { label: "Subjects", href: "/subjects" },
        { label: data.subject, href: `/subjects/${data.subject}` },
        { label: data.courseNumber },
      ]}
    />

    <!-- Header -->
    <div class="mb-6">
      <div class="flex flex-wrap items-center gap-3">
        <h1 class="text-2xl font-bold">
          {representative.title}
        </h1>
        <span
          class="inline-flex items-center px-2 py-0.5 text-xs font-medium rounded
                 bg-muted text-muted-foreground"
        >
          {termDescription}
        </span>
      </div>

      <!-- Metadata bar -->
      <div class="flex flex-wrap items-center gap-x-3 gap-y-1 mt-2 text-sm">
        <span class="inline-flex items-center gap-1.5">
          <span class="text-muted-foreground text-xs">Credits</span>
          <span class="text-foreground">{formatCreditHours(representative)}</span>
        </span>

        {#if representative.instructionalMethod}
          <span class="text-border">|</span>
          <span class="inline-flex items-center gap-1.5">
            <span class="text-muted-foreground text-xs">Delivery</span>
            <span class="text-foreground">
              {getInstructionalMethodLabel(representative.instructionalMethod, "detail")}
            </span>
          </span>
        {/if}

        {#if representative.attributes.length > 0}
          <span class="text-border">|</span>
          <span class="inline-flex items-center gap-1.5">
            <span class="text-muted-foreground text-xs">Attributes</span>
            {#each representative.attributes as attr (attr)}
              <span
                class="inline-flex text-xs font-medium bg-muted border border-border rounded px-1.5 py-0.5 text-muted-foreground"
              >
                {getAttributeLabel(attr, "filter")}
              </span>
            {/each}
          </span>
        {/if}
      </div>
    </div>

    <!-- Sections table -->
    <section class="mb-8">
      <h2 class="text-lg font-semibold mb-3">
        Sections
        <span class="text-sm font-normal text-muted-foreground ml-1">
          ({sections.length})
        </span>
      </h2>
      <CourseTable
        courses={sections}
        loading={false}
        bind:columnVisibility
      />
    </section>

    <!-- Instructors panel -->
    {#if uniqueInstructors.length > 0}
      <section class="mb-8">
        <h2 class="text-lg font-semibold mb-3">Instructors</h2>
        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
          {#each uniqueInstructors as instructor (instructor.instructorId)}
            <div class="flex items-center gap-3 border border-border rounded-lg px-4 py-3 bg-card">
              <div class="min-w-0 flex-1">
                {#if instructor.slug != null}
                  <a
                    href="/instructors/{instructor.slug}"
                    class="font-medium text-sm hover:underline truncate block"
                  >
                    {formatInstructorName(instructor)}
                  </a>
                {:else}
                  <span class="font-medium text-sm truncate block">
                    {formatInstructorName(instructor)}
                  </span>
                {/if}
              </div>
              {#if instructor.rating}
                <ScorePopover
                  rating={instructor.rating}
                  rmp={instructor.rmp}
                  bluebook={instructor.bluebook}
                  size="xs"
                />
              {/if}
            </div>
          {/each}
        </div>
      </section>
    {/if}

    <Footer />
  </div>
</div>
