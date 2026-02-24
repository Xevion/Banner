<script lang="ts">
import { client } from "$lib/api";
import type {
  BluebookLinkDetail,
  BluebookLinkListItem,
  BluebookLinkStats,
  BluebookMatchResponse,
  InstructorListItem,
} from "$lib/bindings";
import FilterCards from "$lib/components/FilterCards.svelte";
import Pagination from "$lib/components/Pagination.svelte";
import ProgressBar from "$lib/components/ProgressBar.svelte";
import SearchInput from "$lib/components/SearchInput.svelte";
import { useDebounceSearch, useRowHighlight } from "$lib/composables";
import { formatInstructorName } from "$lib/course";
import type { FilterCard, ProgressSegment, StatusBadge } from "$lib/ui";
import { getBadge } from "$lib/ui";
import { Check, ChevronRight, LoaderCircle, RefreshCw, Search, X } from "@lucide/svelte";
import { onDestroy, untrack } from "svelte";
import { fade, slide } from "svelte/transition";
import type { PageProps } from "./$types";

let { data }: PageProps = $props();

let links = $state<BluebookLinkListItem[]>(untrack(() => data.links?.links ?? []));
let stats = $state<BluebookLinkStats>(
  untrack(() => data.links?.stats ?? { total: 0, auto: 0, pending: 0, approved: 0, rejected: 0 })
);
let totalCount = $state(untrack(() => data.links?.total ?? 0));
let currentPage = $state(1);
let perPage = $state(25);
let activeFilter = $state<string | undefined>(undefined);
let error = $state<string | null>(untrack(() => data.error));
let loading = $state(false);

// Expanded row detail
let expandedId = $state<number | null>(null);
let detail = $state<BluebookLinkDetail | null>(null);
let detailLoading = $state(false);
let detailError = $state<string | null>(null);

// Action states
let actionLoading = $state<string | null>(null);
let matchLoading = $state(false);
let matchResult = $state<{ message: string; isError: boolean } | null>(null);

// Row highlight tracking
const highlight = useRowHighlight();

// Instructor search for manual assignment
let instructorSearchQuery = $state("");
let instructorSearchResults = $state<InstructorListItem[]>([]);
let instructorSearchLoading = $state(false);
let instructorSearchTimeout: ReturnType<typeof setTimeout> | undefined;

// Debounced search
let searchQuery = $state("");
const search = useDebounceSearch((q) => {
  searchQuery = q;
  currentPage = 1;
  expandedId = null;
  void fetchLinks();
});

const filterCards: FilterCard<BluebookLinkStats>[] = [
  {
    label: "Total",
    value: undefined,
    stat: "total",
    textColor: "text-muted-foreground",
    ringColor: "ring-primary",
  },
  {
    label: "Auto",
    value: "auto",
    stat: "auto",
    textColor: "text-blue-600 dark:text-blue-400",
    ringColor: "ring-blue-500",
  },
  {
    label: "Pending",
    value: "pending",
    stat: "pending",
    textColor: "text-amber-600 dark:text-amber-400",
    ringColor: "ring-amber-500",
  },
  {
    label: "Approved",
    value: "approved",
    stat: "approved",
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

const progressSegments: ProgressSegment<BluebookLinkStats>[] = [
  { stat: "auto", color: "bg-blue-500", label: "Auto" },
  { stat: "pending", color: "bg-amber-500", label: "Pending" },
  { stat: "approved", color: "bg-green-500", label: "Approved" },
  { stat: "rejected", color: "bg-red-500", label: "Rejected" },
];

let totalPages = $derived(Math.max(1, Math.ceil(totalCount / perPage)));

async function fetchLinks() {
  loading = true;
  error = null;
  highlight.clear();
  const result = await client.getAdminBluebookLinks({
    status: activeFilter ?? null,
    search: searchQuery || null,
    page: currentPage,
    perPage: perPage,
  });
  if (result.isErr) {
    error = result.error.message;
  } else {
    links = result.value.links;
    totalCount = result.value.total;
    stats = result.value.stats;
  }
  loading = false;
}

async function fetchDetail(id: number) {
  detailLoading = true;
  detailError = null;
  detail = null;
  instructorSearchQuery = "";
  instructorSearchResults = [];
  const result = await client.getAdminBluebookLink(id);
  if (result.isErr) {
    detailError = result.error.message;
  } else {
    detail = result.value;
  }
  detailLoading = false;
}

onDestroy(() => {
  clearTimeout(instructorSearchTimeout);
  highlight.clear();
});

function setFilter(value: string | undefined) {
  activeFilter = value;
  currentPage = 1;
  expandedId = null;
  void fetchLinks();
}

function clearAllFilters() {
  activeFilter = undefined;
  search.clear();
}

function goToPage(page: number) {
  if (page < 1 || page > totalPages) return;
  currentPage = page;
  expandedId = null;
  void fetchLinks();
}

async function toggleExpand(id: number) {
  if (expandedId === id) {
    expandedId = null;
    detail = null;
    return;
  }
  expandedId = id;
  await fetchDetail(id);
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === "Escape" && expandedId !== null) {
    expandedId = null;
    detail = null;
  }
}

function updateLocalStatus(linkId: number, newStatus: string) {
  links = links.map((l) => (l.id === linkId ? { ...l, status: newStatus } : l));
  highlight.mark(linkId);
}

function matchesFilter(status: string): boolean {
  if (!activeFilter) return true;
  return status === activeFilter;
}

async function handleApprove(linkId: number) {
  actionLoading = `approve-${linkId}`;
  const result = await client.approveBluebookLink(linkId);
  if (result.isErr) {
    detailError = result.error.message;
  } else {
    updateLocalStatus(linkId, "approved");
    if (detail?.id === linkId) {
      detail = { ...detail, status: "approved" };
    }
  }
  actionLoading = null;
}

async function handleReject(linkId: number) {
  actionLoading = `reject-${linkId}`;
  const result = await client.rejectBluebookLink(linkId);
  if (result.isErr) {
    detailError = result.error.message;
  } else {
    updateLocalStatus(linkId, "rejected");
    if (detail?.id === linkId) {
      detail = { ...detail, status: "rejected" };
    }
  }
  actionLoading = null;
}

async function handleAssign(linkId: number, instructorId: number) {
  actionLoading = `assign-${linkId}`;
  const result = await client.assignBluebookLink(linkId, instructorId);
  if (result.isErr) {
    detailError = result.error.message;
  } else {
    instructorSearchQuery = "";
    instructorSearchResults = [];
    await fetchDetail(linkId);
    await fetchLinks();
  }
  actionLoading = null;
}

async function handleAutoMatch() {
  matchLoading = true;
  matchResult = null;
  const result = await client.runBluebookMatching();
  if (result.isErr) {
    matchResult = { message: result.error.message, isError: true };
  } else {
    const res: BluebookMatchResponse = result.value;
    matchResult = {
      message: `Matched ${res.totalNames} names: ${res.autoMatched} auto, ${res.pendingReview} pending, ${res.noMatch} no match (${res.deletedStale} stale cleared, ${res.skippedManual} manual preserved)`,
      isError: false,
    };
    await fetchLinks();
  }
  matchLoading = false;
}

async function doInstructorSearch() {
  const q = instructorSearchQuery.trim();
  if (q.length < 2) {
    instructorSearchResults = [];
    return;
  }
  instructorSearchLoading = true;
  const result = await client.getAdminInstructors({ search: q, perPage: 10 });
  if (result.isOk) {
    instructorSearchResults = result.value.instructors;
  }
  instructorSearchLoading = false;
}

function handleInstructorSearch() {
  clearTimeout(instructorSearchTimeout);
  instructorSearchTimeout = setTimeout(() => {
    void doInstructorSearch();
  }, 300);
}

const BADGES: Record<string, StatusBadge> = {
  auto: { label: "Auto", classes: "bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200" },
  pending: {
    label: "Pending",
    classes: "bg-amber-100 text-amber-800 dark:bg-amber-900 dark:text-amber-200",
  },
  approved: {
    label: "Approved",
    classes: "bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200",
  },
  rejected: {
    label: "Rejected",
    classes: "bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-200",
  },
};

function formatConfidence(confidence: number | null): string {
  if (confidence === null) return "\u2014";
  return `${(confidence * 100).toFixed(0)}%`;
}
</script>

<svelte:head>
  <title>BlueBook Matching | Banner</title>
</svelte:head>

<svelte:window onkeydown={handleKeydown} />

<!-- Header -->
<div class="flex items-center gap-3 mb-4">
  <h1 class="text-lg font-semibold text-foreground">BlueBook Matching</h1>
  <div class="flex-1"></div>

  <!-- Search -->
  <SearchInput
    bind:value={search.input}
    placeholder="Search by name..."
    onSearch={search.trigger}
    onClear={() => search.clear()}
  />

  <!-- Run Auto-Match -->
  <button
    onclick={handleAutoMatch}
    disabled={matchLoading}
    class="inline-flex items-center gap-1.5 rounded-md bg-muted px-3 py-1.5 text-sm font-medium
           text-foreground hover:bg-accent transition-colors disabled:opacity-50 cursor-pointer"
  >
    <RefreshCw size={14} class={matchLoading ? "animate-spin" : ""} />
    Run Auto-Match
  </button>
</div>

<!-- Auto-match result (dismissable) -->
{#if matchResult}
  <div
    class="mb-4 rounded-md px-3 py-2 text-sm flex items-center justify-between gap-2
           {matchResult.isError
      ? 'bg-destructive/10 text-destructive'
      : 'bg-muted text-muted-foreground'}"
    transition:fade={{ duration: 150 }}
  >
    <span>{matchResult.message}</span>
    <button
      onclick={() => (matchResult = null)}
      class="text-muted-foreground hover:text-foreground transition-colors cursor-pointer shrink-0"
      aria-label="Dismiss"
    >
      <X size={14} />
    </button>
  </div>
{/if}

<!-- Error -->
{#if error}
  <div
    class="mb-4 rounded-md bg-destructive/10 px-3 py-2 text-sm text-destructive"
    transition:fade={{ duration: 150 }}
  >
    {error}
  </div>
{/if}

{#if loading && links.length === 0}
  <!-- Skeleton stats cards -->
  <div class="mb-4 grid grid-cols-2 sm:grid-cols-5 gap-3">
    {#each Array(5) as _stat, i (i)}
      <div class="bg-card border-border rounded-lg border p-3">
        <div class="h-3 w-16 animate-pulse rounded bg-muted mb-2"></div>
        <div class="h-6 w-12 animate-pulse rounded bg-muted"></div>
      </div>
    {/each}
  </div>
  <div class="bg-muted mb-6 h-2 rounded-full overflow-hidden"></div>
  <!-- Skeleton table rows -->
  <div class="bg-card border-border overflow-hidden rounded-lg border">
    <div>
      {#each Array(8) as _row, i (i)}
        <div class="flex items-center gap-4 px-4 py-3{i > 0 ? ' border-t border-border' : ''}">
          <div class="flex flex-col gap-y-1.5 flex-1">
            <div class="h-4 w-40 animate-pulse rounded bg-muted"></div>
            <div class="h-3 w-28 animate-pulse rounded bg-muted"></div>
          </div>
          <div class="h-5 w-16 animate-pulse rounded-full bg-muted"></div>
          <div class="h-4 w-20 animate-pulse rounded bg-muted"></div>
          <div class="h-4 w-12 animate-pulse rounded bg-muted"></div>
          <div class="h-6 w-16 animate-pulse rounded bg-muted"></div>
        </div>
      {/each}
    </div>
  </div>
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

    {#if links.length === 0}
      <div class="py-12 text-center">
        {#if searchQuery || activeFilter}
          <p class="text-muted-foreground text-sm">No links match your filters.</p>
          <button
            onclick={clearAllFilters}
            class="mt-2 text-sm text-primary hover:underline cursor-pointer"
          >
            Clear all filters
          </button>
        {:else}
          <p class="text-muted-foreground text-sm">No BlueBook links found.</p>
        {/if}
      </div>
    {:else}
      <div class="bg-card border-border overflow-hidden rounded-lg border">
        <table class="w-full text-sm">
          <thead>
            <tr class="border-border border-b text-left text-muted-foreground">
              <th class="px-4 py-2.5 font-medium">BlueBook Name</th>
              <th class="px-4 py-2.5 font-medium">Subject</th>
              <th class="px-4 py-2.5 font-medium text-center">Evals</th>
              <th class="px-4 py-2.5 font-medium">Matched To</th>
              <th class="px-4 py-2.5 font-medium text-center">Confidence</th>
              <th class="px-4 py-2.5 font-medium">Status</th>
              <th class="px-4 py-2.5 font-medium text-right">Actions</th>
            </tr>
          </thead>
          <tbody>
            {#each links as link (link.id)}
              {@const badge = getBadge(BADGES, link.status)}
              {@const isExpanded = expandedId === link.id}
              {@const isStale = !matchesFilter(link.status)}
              {@const isHighlighted = highlight.has(link.id)}
              <tr
                class="border-border border-b cursor-pointer transition-colors duration-700
                       {isExpanded ? 'bg-muted/30' : 'hover:bg-muted/50'}
                       {isHighlighted ? 'bg-primary/10' : ''}
                       {isStale && !isHighlighted ? 'opacity-60' : ''}"
                onclick={() => toggleExpand(link.id)}
              >
                <td class="px-4 py-2.5">
                  <div class="font-medium text-foreground">{link.instructorName}</div>
                </td>
                <td class="px-4 py-2.5">
                  {#if link.subject}
                    <span class="rounded bg-muted px-1.5 py-0.5 text-xs font-medium"
                      >{link.subject}</span
                    >
                  {:else}
                    <span class="text-muted-foreground text-xs">All</span>
                  {/if}
                </td>
                <td class="px-4 py-2.5 text-center tabular-nums text-muted-foreground">
                  {link.evalCount}
                </td>
                <td class="px-4 py-2.5">
                  {#if link.instructorDisplayName}
                    <span class="text-foreground"
                      >{formatInstructorName(link.instructorDisplayName)}</span
                    >
                  {:else}
                    <span class="text-muted-foreground text-xs">Unmatched</span>
                  {/if}
                </td>
                <td class="px-4 py-2.5 text-center tabular-nums text-muted-foreground">
                  {formatConfidence(link.confidence)}
                </td>
                <td class="px-4 py-2.5">
                  <span
                    class="inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium transition-colors duration-300 {badge.classes}"
                  >
                    {badge.label}
                  </span>
                </td>
                <td class="px-4 py-2.5 text-right">
                  <div class="inline-flex items-center gap-1">
                    {#if (link.status === "pending" || link.status === "auto") && link.instructorId !== null}
                      <button
                        onclick={(e) => {
                          e.stopPropagation();
                          void handleApprove(link.id);
                        }}
                        disabled={actionLoading !== null}
                        class="rounded p-1 text-green-600 hover:bg-green-100 dark:hover:bg-green-900/30
                               transition-colors disabled:opacity-50 cursor-pointer"
                        title="Approve match"
                      >
                        {#if actionLoading === `approve-${link.id}`}
                          <LoaderCircle size={16} class="animate-spin" />
                        {:else}
                          <Check size={16} />
                        {/if}
                      </button>
                      <button
                        onclick={(e) => {
                          e.stopPropagation();
                          void handleReject(link.id);
                        }}
                        disabled={actionLoading !== null}
                        class="rounded p-1 text-red-600 hover:bg-red-100 dark:hover:bg-red-900/30
                               transition-colors disabled:opacity-50 cursor-pointer"
                        title="Reject match"
                      >
                        {#if actionLoading === `reject-${link.id}`}
                          <LoaderCircle size={16} class="animate-spin" />
                        {:else}
                          <X size={16} />
                        {/if}
                      </button>
                    {/if}
                    <button
                      onclick={(e) => {
                        e.stopPropagation();
                        void toggleExpand(link.id);
                      }}
                      class="rounded p-1 text-muted-foreground hover:bg-muted transition-colors cursor-pointer"
                      title={isExpanded ? "Collapse" : "Expand details"}
                      aria-expanded={isExpanded}
                    >
                      <ChevronRight
                        size={16}
                        class="transition-transform duration-200 {isExpanded ? 'rotate-90' : ''}"
                      />
                    </button>
                  </div>
                </td>
              </tr>

              <!-- Expanded detail panel -->
              {#if isExpanded}
                <tr class="border-border border-b bg-muted/20">
                  <td colspan="7" class="p-0 overflow-hidden">
                    <div transition:slide={{ duration: 200 }} class="p-4">
                      {#if detailLoading}
                        <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
                          <div class="flex flex-col gap-y-3 animate-pulse">
                            <div class="h-4 w-20 rounded bg-muted"></div>
                            <div class="flex flex-col gap-y-2">
                              <div class="h-3 w-36 rounded bg-muted"></div>
                              <div class="h-3 w-44 rounded bg-muted"></div>
                              <div class="h-3 w-28 rounded bg-muted"></div>
                            </div>
                          </div>
                          <div class="lg:col-span-2 flex flex-col gap-y-3 animate-pulse">
                            <div class="h-4 w-32 rounded bg-muted"></div>
                            <div class="flex flex-col gap-y-2">
                              <div class="h-20 rounded bg-muted"></div>
                              <div class="h-20 rounded bg-muted"></div>
                            </div>
                          </div>
                        </div>
                      {:else if detailError}
                        <div class="rounded-md bg-destructive/10 px-3 py-2 text-sm text-destructive">
                          {detailError}
                        </div>
                      {:else if detail}
                        <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
                          <!-- Link info -->
                          <div class="flex flex-col gap-y-3">
                            <h3 class="font-medium text-foreground text-sm">Link Info</h3>
                            <dl class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1.5 text-sm">
                              <dt class="text-muted-foreground">Name</dt>
                              <dd class="text-foreground">{detail.instructorName}</dd>

                              <dt class="text-muted-foreground">Subject</dt>
                              <dd class="text-foreground">{detail.subject ?? "All"}</dd>

                              <dt class="text-muted-foreground">Confidence</dt>
                              <dd class="text-foreground tabular-nums">
                                {formatConfidence(detail.confidence)}
                              </dd>

                              <dt class="text-muted-foreground">Evals</dt>
                              <dd class="text-foreground tabular-nums">{detail.evalCount}</dd>

                              <dt class="text-muted-foreground">Status</dt>
                              <dd>
                                <span
                                  class="inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium {getBadge(BADGES, detail.status).classes}"
                                >
                                  {getBadge(BADGES, detail.status).label}
                                </span>
                              </dd>
                            </dl>

                            <!-- Approve/Reject for auto/pending with proposed match -->
                            {#if (detail.status === "pending" || detail.status === "auto") && detail.instructorId !== null}
                              <div class="mt-2 flex flex-col gap-y-2">
                                <div class="text-sm text-muted-foreground">
                                  Proposed match: <span class="text-foreground font-medium"
                                    >{detail.instructorDisplayName
                                      ? formatInstructorName(detail.instructorDisplayName)
                                      : "Unknown"}</span
                                  >
                                </div>
                                <div class="flex gap-2">
                                  <button
                                    onclick={(e) => {
                                      e.stopPropagation();
                                      void handleApprove(detail!.id);
                                    }}
                                    disabled={actionLoading !== null}
                                    class="inline-flex items-center gap-1 rounded-md bg-green-100 px-2.5 py-1
                                           text-xs font-medium text-green-700 hover:bg-green-200
                                           dark:bg-green-900/30 dark:text-green-400 dark:hover:bg-green-900/50
                                           transition-colors disabled:opacity-50 cursor-pointer"
                                  >
                                    <Check size={12} /> Approve
                                  </button>
                                  <button
                                    onclick={(e) => {
                                      e.stopPropagation();
                                      void handleReject(detail!.id);
                                    }}
                                    disabled={actionLoading !== null}
                                    class="inline-flex items-center gap-1 rounded-md bg-red-100 px-2.5 py-1
                                           text-xs font-medium text-red-700 hover:bg-red-200
                                           dark:bg-red-900/30 dark:text-red-400 dark:hover:bg-red-900/50
                                           transition-colors disabled:opacity-50 cursor-pointer"
                                  >
                                    <X size={12} /> Reject
                                  </button>
                                </div>
                              </div>
                            {/if}

                            <!-- Manual instructor search for unmatched auto/pending links -->
                            {#if (detail.status === "pending" || detail.status === "auto") && detail.instructorId === null}
                              <div class="mt-2 flex flex-col gap-y-2">
                                <div class="text-xs text-muted-foreground font-medium">
                                  Assign Instructor
                                </div>
                                <div class="relative">
                                  <Search
                                    size={12}
                                    class="absolute left-2 top-1/2 -translate-y-1/2 text-muted-foreground pointer-events-none"
                                  />
                                  <input
                                    type="text"
                                    placeholder="Search instructors..."
                                    bind:value={instructorSearchQuery}
                                    oninput={handleInstructorSearch}
                                    onclick={(e) => e.stopPropagation()}
                                    class="bg-background border-border rounded-md border pl-7 pr-3 py-1.5 text-xs text-foreground
                                           placeholder:text-muted-foreground outline-none focus:ring-1 focus:ring-ring w-full transition-shadow"
                                  />
                                  {#if instructorSearchLoading}
                                    <LoaderCircle
                                      size={12}
                                      class="absolute right-2 top-1/2 -translate-y-1/2 animate-spin text-muted-foreground"
                                    />
                                  {/if}
                                </div>
                                {#if instructorSearchResults.length > 0}
                                  <div
                                    class="bg-card border-border rounded-md border max-h-40 overflow-y-auto"
                                  >
                                    {#each instructorSearchResults as instructor (instructor.id)}
                                      <button
                                        onclick={(e) => {
                                          e.stopPropagation();
                                          void handleAssign(detail!.id, instructor.id);
                                        }}
                                        disabled={actionLoading !== null}
                                        class="w-full text-left px-3 py-1.5 text-xs hover:bg-muted transition-colors
                                               disabled:opacity-50 cursor-pointer border-b border-border last:border-b-0"
                                      >
                                        <div class="font-medium text-foreground">
                                          {formatInstructorName(instructor.displayName)}
                                        </div>
                                        {#if instructor.email}
                                          <div class="text-muted-foreground">{instructor.email}</div>
                                        {/if}
                                      </button>
                                    {/each}
                                  </div>
                                {/if}
                              </div>
                            {/if}
                          </div>

                          <!-- Courses -->
                          <div class="lg:col-span-2 flex flex-col gap-y-3">
                            <h3 class="font-medium text-foreground text-sm">
                              Courses
                              <span class="text-muted-foreground font-normal"
                                >({detail.courses.length})</span
                              >
                            </h3>

                            {#if detail.courses.length === 0}
                              <p class="text-muted-foreground text-sm py-2">
                                No associated courses.
                              </p>
                            {:else}
                              <div class="max-h-60 overflow-y-auto">
                                <table class="w-full text-xs">
                                  <thead>
                                    <tr
                                      class="border-border border-b text-left text-muted-foreground"
                                    >
                                      <th class="px-3 py-1.5 font-medium">Subject</th>
                                      <th class="px-3 py-1.5 font-medium">Course</th>
                                      <th class="px-3 py-1.5 font-medium">Term</th>
                                      <th class="px-3 py-1.5 font-medium text-center"
                                        >Instructor Rating</th
                                      >
                                      <th class="px-3 py-1.5 font-medium text-center"
                                        >Course Rating</th
                                      >
                                    </tr>
                                  </thead>
                                  <tbody>
                                    {#each detail.courses as course, i (
                                      `${course.subject}-${course.courseNumber}-${course.term}-${i}`
                                    )}
                                      <tr class="border-border border-b last:border-b-0">
                                        <td class="px-3 py-1.5">{course.subject}</td>
                                        <td class="px-3 py-1.5">{course.courseNumber}</td>
                                        <td class="px-3 py-1.5">{course.term}</td>
                                        <td class="px-3 py-1.5 text-center tabular-nums">
                                          {course.instructorRating?.toFixed(1) ?? "\u2014"}
                                        </td>
                                        <td class="px-3 py-1.5 text-center tabular-nums">
                                          {course.courseRating?.toFixed(1) ?? "\u2014"}
                                        </td>
                                      </tr>
                                    {/each}
                                  </tbody>
                                </table>
                              </div>
                            {/if}
                          </div>
                        </div>
                      {/if}
                    </div>
                  </td>
                </tr>
              {/if}
            {/each}
          </tbody>
        </table>
      </div>

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
