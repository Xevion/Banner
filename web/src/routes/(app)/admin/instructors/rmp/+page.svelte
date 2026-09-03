<script lang="ts">
import { client } from "$lib/api";
import type {
  CandidateResponse,
  InstructorDetailResponse,
  InstructorListItem,
  InstructorStats,
  RmpMatchStatus,
} from "$lib/bindings";
import ActionResultBanner from "$lib/components/ActionResultBanner.svelte";
import FilterCards from "$lib/components/FilterCards.svelte";
import MatchListSkeleton from "$lib/components/MatchListSkeleton.svelte";
import MatchPageHeader from "$lib/components/MatchPageHeader.svelte";
import MatchTable from "$lib/components/MatchTable.svelte";
import Pagination from "$lib/components/Pagination.svelte";
import ProgressBar from "$lib/components/ProgressBar.svelte";
import SimpleTooltip from "$lib/components/SimpleTooltip.svelte";
import { useDebounceSearch, useExpandableDetail, useRowHighlight } from "$lib/composables";
import { formatInstructorName, formatYearRange, ratingStyle } from "$lib/course";
import { themeStore } from "$lib/stores/theme.svelte";
import type { FilterCard, MatchColumn, ProgressSegment, StatusBadge } from "$lib/ui";
import { getBadge } from "$lib/ui";
import { Check, LoaderCircle, X } from "@lucide/svelte";
import { onDestroy, untrack } from "svelte";
import { SvelteMap } from "svelte/reactivity";
import { fade } from "svelte/transition";
import type { PageProps } from "./$types";
import CandidateCard from "./CandidateCard.svelte";

let { data }: PageProps = $props();

// Build initial subject map from load data
function buildSubjectMap(
  subjects: { code: string; description: string }[]
): SvelteMap<string, string> {
  const map = new SvelteMap<string, string>();
  for (const entry of subjects) {
    map.set(entry.code, entry.description);
  }
  return map;
}

let subjectMap = $state(untrack(() => buildSubjectMap(data.subjects)));
let instructors = $state<InstructorListItem[]>(untrack(() => data.instructors?.instructors ?? []));
let stats = $state<InstructorStats>(
  untrack(
    () =>
      data.instructors?.stats ?? {
        total: 0,
        unmatched: 0,
        pending: 0,
        auto: 0,
        confirmed: 0,
        rejected: 0,
        withCandidates: 0,
      }
  )
);
let totalCount = $state(untrack(() => data.instructors?.total ?? 0));
let currentPage = $state(1);
let perPage = $state(25);
let activeFilter = $state<string | undefined>(undefined);
let error = $state<string | null>(untrack(() => data.error));
let loading = $state(false);

// Action states
let actionLoading = $state<string | null>(null);
let rescoreLoading = $state(false);
let rescoreResult = $state<{ message: string; isError: boolean } | null>(null);

// Reject-all inline confirmation
let showRejectConfirm = $state<number | null>(null);

// Row highlight tracking
const highlight = useRowHighlight();

// Expanded row detail
const expand = useExpandableDetail<InstructorDetailResponse>({
  fetcher: (id) => client.getAdminInstructor(id),
  onCollapse: () => {
    showRejectConfirm = null;
  },
});

// Debounced search
let searchQuery = $state("");
const search = useDebounceSearch((q) => {
  searchQuery = q;
  currentPage = 1;
  expand.collapse();
  void fetchInstructors();
});

const columns: MatchColumn[] = [
  { label: "Name" },
  { label: "Status" },
  { label: "Top Candidate" },
  { label: "Candidates", class: "text-center" },
];

const filterCards: FilterCard<InstructorStats>[] = [
  {
    label: "No Candidates",
    value: "unmatched",
    stat: "unmatched",
    textColor: "text-slate-500 dark:text-slate-400",
    ringColor: "ring-slate-400",
  },
  {
    label: "Pending",
    value: "pending",
    stat: "pending",
    textColor: "text-orange-600 dark:text-orange-400",
    ringColor: "ring-orange-500",
  },
  {
    label: "Auto",
    value: "auto",
    stat: "auto",
    textColor: "text-blue-600 dark:text-blue-400",
    ringColor: "ring-blue-500",
  },
  {
    label: "Confirmed",
    value: "confirmed",
    stat: "confirmed",
    textColor: "text-green-600 dark:text-green-400",
    ringColor: "ring-green-500",
  },
  {
    label: "Rejected",
    value: "rejected",
    stat: "rejected",
    textColor: "text-red-600 dark:text-red-400",
    ringColor: "ring-red-500",
  },
];

const progressSegments: ProgressSegment<InstructorStats>[] = [
  { stat: "auto", color: "bg-blue-500", label: "Auto" },
  { stat: "confirmed", color: "bg-green-500", label: "Confirmed" },
  { stat: "pending", color: "bg-orange-500", label: "Pending" },
  { stat: "unmatched", color: "bg-slate-400", label: "No Candidates" },
  { stat: "rejected", color: "bg-red-500", label: "Rejected" },
];

let matchedLegacyIds = $derived(
  new Set(expand.detail?.currentMatches.map((m: { legacyId: number }) => m.legacyId) ?? [])
);

let totalPages = $derived(Math.max(1, Math.ceil(totalCount / perPage)));

async function fetchInstructors() {
  loading = true;
  error = null;
  highlight.clear();
  const result = await client.getAdminInstructors({
    status: activeFilter,
    search: searchQuery || undefined,
    page: currentPage,
    perPage: perPage,
  });
  if (result.isErr) {
    error = result.error.message;
  } else {
    const res = result.value;
    instructors = res.instructors;
    totalCount = res.total;
    stats = res.stats;
  }
  loading = false;
}

onDestroy(() => {
  highlight.clear();
});

function setFilter(value: string | undefined) {
  activeFilter = value;
  currentPage = 1;
  expand.collapse();
  void fetchInstructors();
}

function clearAllFilters() {
  activeFilter = undefined;
  search.clear();
}

function goToPage(page: number) {
  if (page < 1 || page > totalPages) return;
  currentPage = page;
  expand.collapse();
  void fetchInstructors();
}

function toggleExpand(id: number) {
  showRejectConfirm = null;
  void expand.toggle(id);
}

function updateLocalStatus(instructorId: number, newStatus: RmpMatchStatus) {
  instructors = instructors.map((i) =>
    i.id === instructorId ? { ...i, rmpMatchStatus: newStatus } : i
  );
  highlight.mark(instructorId);
}

function matchesFilter(status: string): boolean {
  if (!activeFilter) return true;
  return status === activeFilter;
}

async function handleMatch(instructorId: number, rmpLegacyId: number) {
  actionLoading = `match-${rmpLegacyId}`;
  const result = await client.matchInstructor(instructorId, rmpLegacyId);
  if (result.isErr) {
    expand.error = result.error.message;
  } else {
    expand.detail = result.value;
    updateLocalStatus(instructorId, "confirmed");
  }
  actionLoading = null;
}

async function handleReject(instructorId: number, rmpLegacyId: number) {
  actionLoading = `reject-${rmpLegacyId}`;
  const result = await client.rejectCandidate(instructorId, rmpLegacyId);
  if (result.isErr) {
    expand.error = result.error.message;
  } else {
    await expand.load(instructorId);
  }
  actionLoading = null;
}

function requestRejectAll(instructorId: number) {
  showRejectConfirm = instructorId;
}

async function confirmRejectAll(instructorId: number) {
  showRejectConfirm = null;
  actionLoading = "reject-all";
  const result = await client.rejectAllCandidates(instructorId);
  if (result.isErr) {
    expand.error = result.error.message;
  } else {
    await expand.load(instructorId);
    updateLocalStatus(instructorId, "rejected");
  }
  actionLoading = null;
}

function cancelRejectAll() {
  showRejectConfirm = null;
}

async function handleUnmatch(instructorId: number, rmpLegacyId: number) {
  actionLoading = `unmatch-${rmpLegacyId}`;
  const result = await client.unmatchInstructor(instructorId, rmpLegacyId);
  if (result.isErr) {
    expand.error = result.error.message;
  } else {
    await expand.load(instructorId);
    updateLocalStatus(instructorId, "unmatched");
  }
  actionLoading = null;
}

async function handleRescore() {
  rescoreLoading = true;
  rescoreResult = null;
  const result = await client.rescoreInstructors();
  if (result.isErr) {
    rescoreResult = {
      message: result.error.message,
      isError: true,
    };
  } else {
    const res = result.value;
    rescoreResult = {
      message: `Rescored: ${res.totalProcessed} processed, ${res.candidatesCreated} candidates, ${res.autoMatched} auto-matched, ${res.pendingReview} pending review`,
      isError: false,
    };
    await fetchInstructors();
  }
  rescoreLoading = false;
}

const BADGES: Record<string, StatusBadge> = {
  unmatched: {
    label: "No Candidates",
    classes: "bg-slate-100 text-slate-600 dark:bg-slate-800 dark:text-slate-300",
  },
  pending: {
    label: "Pending",
    classes: "bg-orange-100 text-orange-800 dark:bg-orange-900 dark:text-orange-200",
  },
  auto: { label: "Auto", classes: "bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200" },
  confirmed: {
    label: "Confirmed",
    classes: "bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200",
  },
  rejected: {
    label: "Rejected",
    classes: "bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-200",
  },
};

function formatScore(score: number): string {
  return (score * 100).toFixed(0);
}
</script>

<svelte:head>
  <title>RMP Matching | Banner</title>
</svelte:head>

<svelte:window onkeydown={expand.handleKeydown} />

<MatchPageHeader
  title="Instructors"
  searchPlaceholder="Search name or email..."
  {search}
  actionLabel="Rescore"
  actionLoading={rescoreLoading}
  onAction={handleRescore}
/>

<ActionResultBanner result={rescoreResult} onDismiss={() => (rescoreResult = null)} />

<!-- Error -->
{#if error}
  <div
    class="mb-4 rounded-md bg-destructive/10 px-3 py-2 text-sm text-destructive"
    transition:fade={{ duration: 150 }}
  >
    {error}
  </div>
{/if}

{#if loading && instructors.length === 0}
  <MatchListSkeleton
    cellClasses={[
      "h-5 w-20 rounded-full",
      "h-4 w-32 rounded",
      "h-4 w-8 rounded",
      "h-6 w-16 rounded",
    ]}
  />
{:else}
  <div class="relative">
    <!-- Loading overlay for refetching -->
    {#if loading}
      <div
        class="absolute inset-0 z-10 flex items-center justify-center bg-background/60 rounded-lg"
        in:fade={{ duration: 100, delay: 150 }}
        out:fade={{ duration: 100 }}
      >
        <LoaderCircle size={24} class="animate-spin text-muted-foreground" />
      </div>
    {/if}

    <!-- Stats / Filter Cards -->
    <FilterCards {stats} cards={filterCards} {activeFilter} onSelect={setFilter} />

    <!-- Progress Bar -->
    <ProgressBar {stats} segments={progressSegments} total={stats.total} />

    {#if instructors.length === 0}
      <div class="py-12 text-center">
        {#if searchQuery || activeFilter}
          <p class="text-muted-foreground text-sm">No instructors match your filters.</p>
          <button
            onclick={clearAllFilters}
            class="mt-2 text-sm text-primary hover:underline cursor-pointer"
          >
            Clear all filters
          </button>
        {:else}
          <p class="text-muted-foreground text-sm">No instructors found.</p>
        {/if}
      </div>
    {:else}
      <MatchTable
        {columns}
        rows={instructors}
        getId={(instructor: InstructorListItem) => instructor.id}
        expandedId={expand.expandedId}
        isStale={(instructor: InstructorListItem) => !matchesFilter(instructor.rmpMatchStatus)}
        isHighlighted={(id) => highlight.has(id)}
        detail={expand.detail}
        detailLoading={expand.loading}
        detailError={expand.error}
        onToggle={toggleExpand}
        {cells}
        {actions}
        {detailPanel}
      />

      <!-- Pagination -->
      <Pagination
        variant="simple"
        currentPage={currentPage}
        {totalCount}
        perPage={perPage}
        onPageChange={goToPage}
      />
    {/if}
  </div>
{/if}

{#snippet cells(instructor: InstructorListItem)}
  {@const badge = getBadge(BADGES, instructor.rmpMatchStatus)}
  <td class="px-4 py-2.5">
    <div class="font-medium text-foreground">
      {formatInstructorName(instructor.displayName)}
    </div>
    {#if instructor.email}
      <div class="text-xs text-muted-foreground">{instructor.email}</div>
    {/if}
  </td>
  <td class="px-4 py-2.5">
    <span
      class="inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium transition-colors duration-300 {badge.classes}"
    >
      {badge.label}
    </span>
  </td>
  <td class="px-4 py-2.5">
    {#if instructor.topCandidate}
      {@const tc = instructor.topCandidate}
      <div class="flex items-center gap-2">
        <span class="text-foreground">{tc.firstName} {tc.lastName}</span>
        {#if tc.avgRating != null}
          <span class="font-semibold tabular-nums" style={ratingStyle(tc.avgRating, themeStore.isDark)}>
            {tc.avgRating.toFixed(1)}
          </span>
        {:else}
          <span class="text-xs text-muted-foreground">N/A</span>
        {/if}
        <span class="text-xs text-muted-foreground tabular-nums">
          ({formatScore(tc.score ?? 0)}%)
        </span>
      </div>
    {:else}
      <span class="text-muted-foreground text-xs">No candidates</span>
    {/if}
  </td>
  <td class="px-4 py-2.5 text-center tabular-nums text-muted-foreground">
    {instructor.candidateCount}
  </td>
{/snippet}

{#snippet actions(instructor: InstructorListItem)}
  {#if instructor.topCandidate && (instructor.rmpMatchStatus === "unmatched" || instructor.rmpMatchStatus === "pending")}
    <button
      onclick={(e) => {
        e.stopPropagation();
        void handleMatch(instructor.id, instructor.topCandidate!.rmpLegacyId);
      }}
      disabled={actionLoading !== null}
      class="rounded p-1 text-green-600 hover:bg-green-100 dark:hover:bg-green-900/30
             transition-colors disabled:opacity-50 cursor-pointer"
      title="Accept top candidate"
    >
      {#if actionLoading === `match-${instructor.topCandidate.rmpLegacyId}`}
        <LoaderCircle size={16} class="animate-spin" />
      {:else}
        <Check size={16} />
      {/if}
    </button>
  {/if}
{/snippet}

{#snippet detailPanel(detail: InstructorDetailResponse)}
  <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
    <!-- Instructor info -->
    <div class="flex flex-col gap-y-3">
      <h3 class="font-medium text-foreground text-sm">Instructor</h3>
      <dl class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1.5 text-sm">
        <dt class="text-muted-foreground">Name</dt>
        <dd class="text-foreground">
          {formatInstructorName(detail.instructor.displayName)}
        </dd>

        {#if detail.instructor.email}
          <dt class="text-muted-foreground">Email</dt>
          <dd class="text-foreground break-all">{detail.instructor.email}</dd>
        {/if}

        <dt class="text-muted-foreground">Courses</dt>
        <dd class="text-foreground tabular-nums">
          {detail.instructor.courseCount}
        </dd>

        {#if detail.instructor.subjectsTaught.length > 0}
          <dt class="text-muted-foreground">Subjects</dt>
          <dd class="flex flex-wrap gap-1">
            {#each detail.instructor.subjectsTaught as subj (subj)}
              {#if subjectMap.has(subj)}
                <SimpleTooltip text={subjectMap.get(subj)!} delay={75}>
                  <span class="rounded bg-muted px-1.5 py-0.5 text-xs font-medium">{subj}</span>
                </SimpleTooltip>
              {:else}
                <span class="rounded bg-muted px-1.5 py-0.5 text-xs font-medium">{subj}</span>
              {/if}
            {/each}
          </dd>
        {/if}

        {#if detail.instructor.teachingYears.length > 0}
          <dt class="text-muted-foreground">Active</dt>
          <dd class="text-foreground text-xs tabular-nums">
            {formatYearRange(detail.instructor.teachingYears)}
          </dd>
        {/if}
      </dl>
    </div>

    <!-- Candidates -->
    <div class="lg:col-span-2 flex flex-col gap-y-3">
      <div class="flex items-center justify-between gap-2">
        <h3 class="font-medium text-foreground text-sm">
          Candidates
          <span class="text-muted-foreground font-normal">({detail.candidates.length})</span>
        </h3>
        {#if detail.candidates.some((c: CandidateResponse) => c.status !== "rejected" && !matchedLegacyIds.has(c.rmpLegacyId))}
          {#if showRejectConfirm === detail.instructor.id}
            <div class="inline-flex items-center gap-2 text-xs" in:fade={{ duration: 100 }}>
              <span class="text-muted-foreground">Reject all candidates?</span>
              <button
                onclick={(e) => {
                  e.stopPropagation();
                  void confirmRejectAll(detail.instructor.id);
                }}
                disabled={actionLoading !== null}
                class="font-medium text-red-600 hover:text-red-700
                       dark:text-red-400 dark:hover:text-red-300
                       cursor-pointer disabled:opacity-50"
              >
                Confirm
              </button>
              <button
                onclick={(e) => {
                  e.stopPropagation();
                  cancelRejectAll();
                }}
                class="text-muted-foreground hover:text-foreground cursor-pointer"
              >
                Cancel
              </button>
            </div>
          {:else}
            <button
              onclick={(e) => {
                e.stopPropagation();
                requestRejectAll(detail.instructor.id);
              }}
              disabled={actionLoading !== null}
              class="inline-flex items-center gap-1 rounded-md bg-red-100 px-2 py-1
                     text-xs font-medium text-red-700 hover:bg-red-200
                     dark:bg-red-900/30 dark:text-red-400 dark:hover:bg-red-900/50
                     transition-colors disabled:opacity-50 cursor-pointer"
            >
              <X size={12} /> Reject All
            </button>
          {/if}
        {/if}
      </div>

      {#if detail.candidates.length === 0}
        <p class="text-muted-foreground text-sm py-2">No candidates available.</p>
      {:else}
        <div class="max-h-80 overflow-y-auto flex flex-col gap-y-2 pr-1">
          {#each detail.candidates as candidate (candidate.id)}
            <CandidateCard
              {candidate}
              isMatched={candidate.status === "matched" ||
                matchedLegacyIds.has(candidate.rmpLegacyId)}
              isRejected={candidate.status === "rejected"}
              disabled={actionLoading !== null}
              {actionLoading}
              isDark={themeStore.isDark}
              onmatch={() => handleMatch(detail.instructor.id, candidate.rmpLegacyId)}
              onreject={() => handleReject(detail.instructor.id, candidate.rmpLegacyId)}
              onunmatch={() => handleUnmatch(detail.instructor.id, candidate.rmpLegacyId)}
            />
          {/each}
        </div>
      {/if}
    </div>
  </div>
{/snippet}
