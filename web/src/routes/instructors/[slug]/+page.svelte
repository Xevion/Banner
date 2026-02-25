<script lang="ts">
import { client } from "$lib/api";
import type {
  CourseResponse,
  PublicInstructorProfileResponse,
  SearchOptionsResponse,
} from "$lib/bindings";
import Footer from "$lib/components/Footer.svelte";
import TermCombobox from "$lib/components/TermCombobox.svelte";
import { buildAttributeMap, setCourseDetailContext } from "$lib/components/course-detail/context";
import { CourseTable } from "$lib/components/course-table";
import { formatInstructorName, ratingStyle, rmpUrl } from "$lib/course";
import { themeStore } from "$lib/stores/theme.svelte";
import { formatNumber } from "$lib/utils";
import { BookOpen, Copy, ExternalLink, Mail, Star } from "@lucide/svelte";
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
const composite = instructor.composite;

let selectedTerm = $state(untrack(() => data.initialTerm ?? ""));
let sections = $state<CourseResponse[]>(untrack(() => data.initialSections ?? []));
let sectionsLoading = $state(false);
let copiedEmail = $state(false);

const allTerms = $derived(data.searchOptions?.terms ?? []);
const instructorTermCodes = $derived(new Set(profile.teachingHistory.map((t) => t.termCode)));
const terms = $derived(allTerms.filter((t) => instructorTermCodes.has(t.code)));
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

function courseSearchUrl(termCode: string, subject: string, courseNumber: string): string {
  const term = terms.find((t) => t.code === termCode);
  const slug = term?.slug ?? termCode;
  return `/?term=${encodeURIComponent(slug)}&subject=${encodeURIComponent(subject)}&q=${encodeURIComponent(subject + " " + courseNumber)}`;
}

function resolveSubject(code: string): string {
  return subjectMap.get(code) ?? code;
}

const displayName = $derived(formatInstructorName(instructor));
</script>

<svelte:head>
  <title>{instructor.displayName} | Banner</title>
</svelte:head>

<div class="min-h-screen flex flex-col items-center px-3 md:px-5 pb-5 pt-20">
  <div class="w-full max-w-6xl flex flex-col pt-2">
    <!-- Breadcrumbs -->
    <nav class="text-xs text-muted-foreground mb-4">
      <a href="/" class="hover:text-foreground transition-colors">Home</a>
      <span class="mx-1">&gt;</span>
      <a href="/instructors" class="hover:text-foreground transition-colors">Instructors</a>
      <span class="mx-1">&gt;</span>
      <span class="text-foreground">{displayName}</span>
    </nav>

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
            <span
              class="inline-block px-2 py-0.5 text-xs font-medium rounded
                     bg-muted text-muted-foreground truncate max-w-48"
            >
              {resolveSubject(subject)}
            </span>
          {/each}
        </div>
      {/if}
    </div>

    <!-- Combined Rating -->
    {#if composite}
      <div class="rounded-lg border border-border bg-card p-5 mb-4">
        <div class="flex items-center gap-4">
          <div
            class="text-3xl font-bold inline-flex items-center gap-1"
            style={ratingStyle(composite.score, themeStore.isDark)}
          >
            {composite.score.toFixed(1)}
            {#if bluebook && !rmp?.avgRating}
              <BookOpen class="size-5 fill-current" />
            {:else}
              <Star class="size-5 fill-current" />
            {/if}
          </div>
          <div>
            <div class="text-sm font-medium">Combined Rating</div>
            <div class="text-xs text-muted-foreground">
              {formatNumber(composite.totalResponses)} total responses
            </div>
          </div>
        </div>
      </div>
    {/if}

    <!-- Rating Source Cards -->
    {#if rmp ?? bluebook}
      <div class="grid grid-cols-1 md:grid-cols-2 gap-4 mb-6">
        {#if bluebook}
          <div class="rounded-lg border border-border bg-card p-5">
            <div class="flex items-center gap-2 mb-3">
              <BookOpen class="size-4 text-muted-foreground" />
              <h3 class="text-sm font-semibold">BlueBook</h3>
            </div>
            <div class="flex items-center gap-6 flex-wrap">
              <div class="text-center">
                <div
                  class="text-2xl font-bold"
                  style={ratingStyle(bluebook.avgInstructorRating, themeStore.isDark)}
                >
                  {bluebook.avgInstructorRating.toFixed(1)}
                </div>
                <div class="text-xs text-muted-foreground mt-0.5">Instructor</div>
              </div>
              {#if bluebook.avgCourseRating != null}
                <div class="text-center">
                  <div class="text-xl font-semibold">{bluebook.avgCourseRating.toFixed(1)}</div>
                  <div class="text-xs text-muted-foreground mt-0.5">Course</div>
                </div>
              {/if}
              <div class="text-center">
                <div class="text-xl font-semibold">{formatNumber(bluebook.totalResponses)}</div>
                <div class="text-xs text-muted-foreground mt-0.5">Responses</div>
              </div>
              <div class="text-center">
                <div class="text-xl font-semibold">{bluebook.evalCount}</div>
                <div class="text-xs text-muted-foreground mt-0.5">Evaluations</div>
              </div>
            </div>
          </div>
        {/if}

        {#if rmp}
          <div class="rounded-lg border border-border bg-card p-5">
            <div class="flex items-center justify-between mb-3">
              <div class="flex items-center gap-2">
                <Star class="size-4 text-muted-foreground" />
                <h3 class="text-sm font-semibold">RateMyProfessors</h3>
              </div>
              <a
                href={rmpUrl(rmp.legacyId)}
                target="_blank"
                rel="noopener"
                class="inline-flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
              >
                View
                <ExternalLink class="size-3" />
              </a>
            </div>
            <div class="flex items-center gap-6 flex-wrap">
              {#if rmp.avgRating != null}
                <div class="text-center">
                  <div
                    class="text-2xl font-bold"
                    style={ratingStyle(rmp.avgRating, themeStore.isDark)}
                  >
                    {rmp.avgRating.toFixed(1)}
                  </div>
                  <div class="text-xs text-muted-foreground mt-0.5">Overall</div>
                </div>
                {#if rmp.avgDifficulty != null}
                  <div class="text-center">
                    <div class="text-xl font-semibold">{rmp.avgDifficulty.toFixed(1)}</div>
                    <div class="text-xs text-muted-foreground mt-0.5">Difficulty</div>
                  </div>
                {/if}
                {#if rmp.wouldTakeAgainPct != null}
                  <div class="text-center">
                    <div class="text-xl font-semibold">{Math.round(rmp.wouldTakeAgainPct)}%</div>
                    <div class="text-xs text-muted-foreground mt-0.5">Would Take Again</div>
                  </div>
                {/if}
                {#if rmp.numRatings != null}
                  <div class="text-center">
                    <div class="text-xl font-semibold">{formatNumber(rmp.numRatings)}</div>
                    <div class="text-xs text-muted-foreground mt-0.5">Ratings</div>
                  </div>
                {/if}
              {:else}
                <span class="text-sm text-muted-foreground">No ratings yet</span>
              {/if}
            </div>
          </div>
        {/if}
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
              {#each profile.teachingHistory as term (term.termCode)}
                {#each term.courses as course, ci (`${term.termCode}-${course.subject}-${course.courseNumber}-${course.title}`)}
                  <tr class="border-t border-border first:border-t-0 hover:bg-muted/20 transition-colors">
                    <td class="px-3 py-2 text-muted-foreground whitespace-nowrap">
                      {#if ci === 0}
                        {term.termDescription}
                      {/if}
                    </td>
                    <td class="px-3 py-2 font-medium whitespace-nowrap">
                      <a
                        href={courseSearchUrl(term.termCode, course.subject, course.courseNumber)}
                        class="hover:underline"
                      >
                        {course.subject} {course.courseNumber}
                      </a>
                    </td>
                    <td class="px-3 py-2 text-muted-foreground truncate max-w-xs">
                      {course.title}
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
