<script lang="ts">
import { client } from "$lib/api";
import type {
  CourseResponse,
  PublicInstructorProfileResponse,
  SearchOptionsResponse,
} from "$lib/bindings";
import ScoreBar from "$lib/components/ScoreBar.svelte";
import Footer from "$lib/components/Footer.svelte";
import TermCombobox from "$lib/components/TermCombobox.svelte";
import { buildAttributeMap, setCourseDetailContext } from "$lib/components/course-detail/context";
import { CourseTable } from "$lib/components/course-table";
import SourceScoreCard from "$lib/components/score/SourceScoreCard.svelte";
import Breadcrumb from "$lib/components/Breadcrumb.svelte";
import { formatInstructorName, rmpUrl } from "$lib/course";
import { Copy, ExternalLink, Mail } from "@lucide/svelte";
import { Tabs } from "bits-ui";
import { untrack } from "svelte";

interface PageData {
  profile: PublicInstructorProfileResponse;
  searchOptions: SearchOptionsResponse | null;
  initialSections: CourseResponse[] | null;
  initialTerm: string | null;
  slug: string;
}

let { data }: { data: PageData } = $props();

const profile = untrack(() => data.profile);
const instructor = profile.instructor;
const rmp = instructor.rmp;
const bluebook = instructor.bluebook;
const rating = instructor.rating;

let selectedTerm = $state(untrack(() => data.initialTerm ?? ""));
let sections = $state<CourseResponse[]>(untrack(() => data.initialSections ?? []));
let sectionsLoading = $state(false);
let copiedEmail = $state(false);

const allTerms = $derived(data.searchOptions?.terms ?? []);
const instructorTermSlugs = $derived(new Set(profile.teachingHistory.map((t) => t.termSlug)));
const terms = $derived(allTerms.filter((t) => instructorTermSlugs.has(t.slug)));
const subjects = $derived(data.searchOptions?.subjects ?? []);
const subjectMap = $derived(
  new Map(subjects.map((s: { code: string; description: string }) => [s.code, s.description]))
);
const attributes = $derived(data.searchOptions?.reference.attributes ?? []);
const attributeMap = $derived(buildAttributeMap(attributes));

setCourseDetailContext({
  get attributeMap() {
    return attributeMap;
  },
  navigateToSection: null,
});

let columnVisibility = $state({ instructor: false });

async function onTermChange() {
  if (!selectedTerm) return;
  sectionsLoading = true;
  const result = await client.getInstructorSections(data.slug, selectedTerm);
  if (result.isOk) {
    sections = result.value;
  } else {
    console.warn("Failed to load instructor sections:", result.error.message);
  }
  sectionsLoading = false;
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

async function copyEmail() {
  if (!instructor.email) return;
  await navigator.clipboard.writeText(instructor.email);
  copiedEmail = true;
  setTimeout(() => (copiedEmail = false), 2000);
}

function resolveSubject(code: string): string {
  return subjectMap.get(code) ?? code;
}

const displayName = $derived(formatInstructorName(instructor));

interface ScoreBarProps {
  score: number;
  rankScore: number;
  ciLower: number;
  ciUpper: number;
  confidence: number;
  source: "both" | "rmp" | "bluebook";
  rmpRating: number | null;
  rmpCount: number;
  bbRating: number | null;
  bbCount: number;
}

const scoreBarProps: ScoreBarProps | null = $derived.by(() => {
  if (!rating) return null;
  return {
    score: rating.score,
    rankScore: rating.rankScore,
    ciLower: rating.ciLower,
    ciUpper: rating.ciUpper,
    confidence: rating.confidence,
    source: rating.source,
    rmpRating: rmp?.avgRating ?? null,
    rmpCount: rmp?.numRatings ?? 0,
    bbRating: bluebook?.avgInstructorRating ?? null,
    bbCount: bluebook?.totalResponses ?? 0,
  };
});
</script>

<svelte:head>
  <title>{instructor.displayName} | Banner</title>
</svelte:head>

<div class="min-h-screen flex flex-col items-center px-3 md:px-5 pb-5 pt-20">
  <div class="w-full max-w-6xl flex flex-col pt-2">
    <Breadcrumb
      items={[
        { label: "Home", href: "/" },
        { label: "Instructors", href: "/instructors" },
        { label: displayName },
      ]}
    />

    <!-- Header -->
    <div class="mb-6">
      <h1 class="text-2xl font-bold">{displayName}</h1>
      {#if instructor.email}
        <div class="flex items-center gap-2 mt-1.5">
          <Mail class="size-3.5 text-muted-foreground" />
          <span class="text-sm text-muted-foreground">{instructor.email}</span>
          <button
            onclick={copyEmail}
            class="text-muted-foreground hover:text-foreground transition-colors"
            title="Copy email"
          >
            <Copy class="size-3.5" />
          </button>
          {#if copiedEmail}
            <span class="text-xs text-green-500">Copied!</span>
          {/if}
        </div>
      {/if}

      {#if instructor.subjects.length > 0}
        <div class="flex flex-wrap gap-1.5 mt-3">
          {#each instructor.subjects as subject (subject)}
            <a
              href="/subjects/{subject}"
              class="inline-block px-2 py-0.5 text-xs font-medium rounded
                     bg-muted text-muted-foreground truncate max-w-48 hover:bg-muted/80 hover:text-foreground transition-colors"
            >
              {resolveSubject(subject)}
            </a>
          {/each}
        </div>
      {/if}
    </div>

    <!-- Rating -->
    {#if scoreBarProps}
      {@const hasRmp = rmp?.avgRating != null}
      {@const hasBb = bluebook != null}
      {@const hasBothSources = hasRmp && hasBb}
      {@const defaultTab = hasRmp ? "rmp" : "bluebook"}
      <div class="rounded-lg border border-border bg-card mb-6">
        <div class="flex flex-col md:flex-row">
          <!-- ScoreBar (left / top on mobile) -->
          <div class="flex-1 px-3 pt-3 pb-2 sm:px-5 sm:pt-4 {hasBb || hasRmp ? 'md:border-r md:border-border' : ''}">
            <div class="text-sm font-medium mb-1">Rating</div>
            <ScoreBar {...scoreBarProps} />
          </div>

          <!-- Source detail tabs (right / bottom on mobile) -->
          {#if hasRmp || hasBb}
            <div class="md:w-[35%] shrink-0 border-t md:border-t-0 border-border">
              {#if hasBothSources}
                <Tabs.Root value={defaultTab}>
                  <Tabs.List class="flex border-b border-border bg-muted/20">
                    <Tabs.Trigger
                      value="rmp"
                      class="flex-1 px-3 py-2 text-xs font-medium text-muted-foreground transition-colors hover:text-foreground hover:bg-muted/40 active:bg-muted/60 data-[state=active]:text-foreground data-[state=active]:border-b-2 data-[state=active]:border-muted-foreground/50 data-[state=active]:-mb-px cursor-pointer"
                    >
                      <span class="inline-flex items-center gap-1">
                        RateMyProfessors
                        {#if rmp?.legacyId != null}
                          <a
                            href={rmpUrl(rmp.legacyId)}
                            target="_blank"
                            rel="noopener"
                            class="text-muted-foreground hover:text-foreground"
                            onclick={(e) => e.stopPropagation()}
                          >
                            <ExternalLink class="size-3" />
                          </a>
                        {/if}
                      </span>
                    </Tabs.Trigger>
                    <Tabs.Trigger
                      value="bluebook"
                      class="flex-1 px-3 py-2 text-xs font-medium text-muted-foreground transition-colors hover:text-foreground hover:bg-muted/40 active:bg-muted/60 data-[state=active]:text-foreground data-[state=active]:border-b-2 data-[state=active]:border-muted-foreground/50 data-[state=active]:-mb-px cursor-pointer"
                    >
                      BlueBook
                    </Tabs.Trigger>
                  </Tabs.List>
                  <Tabs.Content value="rmp" class="p-4">
                    {#if rmp}
                      <SourceScoreCard rating={rating!} source="rmp" {rmp} inline />
                    {/if}
                  </Tabs.Content>
                  <Tabs.Content value="bluebook" class="p-4">
                    {#if bluebook}
                      <SourceScoreCard rating={rating!} source="bluebook" {bluebook} inline />
                    {/if}
                  </Tabs.Content>
                </Tabs.Root>
              {:else}
                <!-- Single source: no tabs, just a label + card -->
                <div class="px-4 py-2 border-b border-border bg-muted/20">
                  <span class="text-xs font-medium text-muted-foreground">
                    {hasRmp ? "RateMyProfessors" : "BlueBook"}
                  </span>
                </div>
                <div class="p-4">
                  {#if hasRmp && rmp}
                    <SourceScoreCard rating={rating!} source="rmp" {rmp} inline />
                  {:else if hasBb && bluebook}
                    <SourceScoreCard rating={rating!} source="bluebook" {bluebook} inline />
                  {/if}
                </div>
              {/if}
            </div>
          {/if}
        </div>
      </div>
    {/if}

    <!-- Current Sections -->
    <section class="mb-8">
      <div class="flex items-center gap-3 mb-3">
        <h2 class="text-lg font-semibold">Sections</h2>
        {#if terms.length > 1}
          <TermCombobox {terms} bind:value={selectedTerm} />
        {:else if terms.length === 1}
          <span class="text-sm text-muted-foreground">{terms[0].description}</span>
        {/if}
      </div>

      {#if sectionsLoading}
        <!-- Skeleton rows while sections load -->
        <div class="rounded-lg border border-border overflow-hidden">
          <div class="animate-pulse">
            {#each Array(5) as _, i (i)}
              <div class="flex gap-4 px-4 py-3 {i > 0 ? 'border-t border-border' : ''}">
                <div class="h-4 w-12 bg-muted rounded"></div>
                <div class="h-4 w-16 bg-muted rounded"></div>
                <div class="h-4 w-40 bg-muted rounded flex-1"></div>
                <div class="h-4 w-24 bg-muted rounded"></div>
                <div class="h-4 w-16 bg-muted rounded"></div>
              </div>
            {/each}
          </div>
        </div>
      {:else if sections.length > 0}
        <CourseTable
          courses={sections}
          loading={false}
          bind:columnVisibility
        />
      {:else}
        <div class="text-center py-8 text-muted-foreground text-sm border border-border rounded-lg">
          {#if terms.length === 0}
            No sections on record for this instructor.
          {:else}
            No sections found for this term.
          {/if}
        </div>
      {/if}
    </section>

    <!-- Teaching History -->
    {#if profile.teachingHistory.length > 0}
      <section class="mb-8">
        <h2 class="text-lg font-semibold mb-3">Teaching History</h2>
        <div class="rounded-lg border border-border overflow-hidden">
          <table class="w-full text-sm">
            <thead>
              <tr class="border-b border-border bg-muted/30">
                <th class="text-left px-3 py-2 font-medium text-muted-foreground">Term</th>
                <th class="text-left px-3 py-2 font-medium text-muted-foreground">Course</th>
                <th class="text-left px-3 py-2 font-medium text-muted-foreground">Title</th>
                <th class="text-right px-3 py-2 font-medium text-muted-foreground">Sections</th>
              </tr>
            </thead>
            <tbody>
              {#each profile.teachingHistory as term (term.termSlug)}
                {#each term.courses as course, ci (`${term.termSlug}-${course.subject}-${course.courseNumber}-${course.title}`)}
                  {@const coursePageUrl = `/courses/${term.termSlug}/${course.subject}/${course.courseNumber}`}
                  <tr class="border-t border-border first:border-t-0 hover:bg-muted/20 transition-colors">
                    <td class="px-3 py-2 text-muted-foreground whitespace-nowrap">
                      {#if ci === 0}
                        {term.termDescription}
                      {/if}
                    </td>
                    <td class="px-3 py-2 font-medium whitespace-nowrap">
                      <a
                        href={coursePageUrl}
                        class="hover:underline hover:text-foreground transition-colors"
                      >
                        {course.subject} {course.courseNumber}
                      </a>
                    </td>
                    <td class="px-3 py-2 text-muted-foreground truncate max-w-xs">
                      <a
                        href={coursePageUrl}
                        class="hover:underline hover:text-foreground transition-colors"
                      >
                        {course.title}
                      </a>
                    </td>
                    <td class="px-3 py-2 text-right tabular-nums text-muted-foreground">
                      {course.sectionCount}
                    </td>
                  </tr>
                {/each}
              {/each}
            </tbody>
          </table>
        </div>
      </section>
    {/if}

    <Footer />
  </div>
</div>
