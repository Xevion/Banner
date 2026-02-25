<script lang="ts">
import type { CandidateResponse } from "$lib/bindings";
import { formatYearRange, ratingStyle, rmpUrl } from "$lib/course";
import { Check, ExternalLink, LoaderCircle, X, XCircle } from "@lucide/svelte";
import ScoreBreakdown from "./ScoreBreakdown.svelte";

let {
  candidate,
  isMatched = false,
  isRejected = false,
  disabled = false,
  actionLoading = null,
  isDark = false,
  onmatch,
  onreject,
  onunmatch,
}: {
  candidate: CandidateResponse;
  isMatched?: boolean;
  isRejected?: boolean;
  disabled?: boolean;
  actionLoading?: string | null;
  isDark?: boolean;
  onmatch?: () => void;
  onreject?: () => void;
  onunmatch?: () => void;
} = $props();

const isPending = $derived(!isMatched && !isRejected);
const isMatchLoading = $derived(actionLoading === `match-${candidate.rmpLegacyId}`);
const isRejectLoading = $derived(actionLoading === `reject-${candidate.rmpLegacyId}`);
const isUnmatchLoading = $derived(actionLoading === `unmatch-${candidate.rmpLegacyId}`);
</script>

<div
  class="rounded-md border p-3 transition-all duration-200
    {isMatched
      ? 'border-l-4 border-l-green-500 bg-green-500/5 border-border'
      : isRejected
        ? 'border-border bg-card opacity-50'
        : 'border-border bg-card hover:shadow-sm'}"
>
  <div class="flex items-start justify-between gap-2">
    <div class="min-w-0">
      <div class="flex items-center gap-2 flex-wrap">
        <span class="font-medium text-foreground text-sm">
          {candidate.firstName} {candidate.lastName}
        </span>
        {#if isMatched}
          <span
            class="text-[10px] rounded px-1.5 py-0.5 bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400 font-medium"
          >
            Matched
          </span>
        {:else if isRejected}
          <span
            class="text-[10px] rounded px-1.5 py-0.5 bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400 font-medium"
          >
            Rejected
          </span>
        {/if}
      </div>
      {#if candidate.department}
        <div class="text-xs text-muted-foreground mt-0.5">{candidate.department}</div>
      {/if}
    </div>

    <div class="flex items-center gap-0.5 shrink-0">
      {#if isMatched}
        <button
          onclick={(e) => {
            e.stopPropagation();
            onunmatch?.();
          }}
          {disabled}
          class="inline-flex items-center gap-1 rounded px-1.5 py-1 text-xs text-red-500 hover:bg-red-100 dark:hover:bg-red-900/30 transition-colors disabled:opacity-50 cursor-pointer"
          title="Remove match"
        >
          {#if isUnmatchLoading}
            <LoaderCircle size={14} class="animate-spin" />
          {:else}
            <XCircle size={14} />
          {/if}
          Unmatch
        </button>
      {:else if isPending}
        <button
          onclick={(e) => {
            e.stopPropagation();
            onmatch?.();
          }}
          {disabled}
          class="rounded p-1 text-green-600 hover:bg-green-100 dark:hover:bg-green-900/30 transition-colors disabled:opacity-50 cursor-pointer"
          title="Accept match"
        >
          {#if isMatchLoading}
            <LoaderCircle size={14} class="animate-spin" />
          {:else}
            <Check size={14} />
          {/if}
        </button>
        <button
          onclick={(e) => {
            e.stopPropagation();
            onreject?.();
          }}
          {disabled}
          class="rounded p-1 text-red-500 hover:bg-red-100 dark:hover:bg-red-900/30 transition-colors disabled:opacity-50 cursor-pointer"
          title="Reject candidate"
        >
          {#if isRejectLoading}
            <LoaderCircle size={14} class="animate-spin" />
          {:else}
            <X size={14} />
          {/if}
        </button>
      {/if}
      <a
        href={rmpUrl(candidate.rmpLegacyId)}
        target="_blank"
        rel="noopener noreferrer"
        onclick={(e) => e.stopPropagation()}
        class="rounded p-1 text-muted-foreground hover:bg-muted hover:text-foreground transition-colors cursor-pointer"
        title="View on RateMyProfessors"
      >
        <ExternalLink size={14} />
      </a>
    </div>
  </div>

  <!-- Rating stats -->
  <div class="mt-2 flex items-center gap-3 text-xs flex-wrap">
    {#if candidate.avgRating != null}
      <span
        class="font-semibold tabular-nums"
        style={ratingStyle(candidate.avgRating, isDark)}
      >
        {candidate.avgRating.toFixed(1)}
      </span>
    {:else}
      <span class="text-muted-foreground">No rating</span>
    {/if}
    {#if candidate.avgDifficulty !== null}
      <span class="text-muted-foreground tabular-nums"
        >{candidate.avgDifficulty.toFixed(1)} diff</span
      >
    {/if}
    <span class="text-muted-foreground tabular-nums">{candidate.numRatings} ratings</span>
    {#if candidate.wouldTakeAgainPct !== null}
      <span class="text-muted-foreground tabular-nums"
        >{candidate.wouldTakeAgainPct.toFixed(0)}% again</span
      >
    {/if}
  </div>

  <!-- Review metadata (subjects + years) -->
  {#if candidate.reviewSubjects.length > 0 || candidate.reviewYears.length > 0}
    <div class="mt-2 flex items-center gap-3 text-xs flex-wrap">
      {#if candidate.reviewSubjects.length > 0}
        <span class="text-muted-foreground">
          Subjects: <span class="text-foreground font-medium">{candidate.reviewSubjects.join(", ")}</span>
        </span>
      {/if}
      {#if candidate.reviewYears.length > 0}
        <span class="text-muted-foreground">
          Reviews: <span class="text-foreground tabular-nums font-medium">{formatYearRange(candidate.reviewYears)}</span>
        </span>
      {/if}
    </div>
  {/if}

  <!-- Score breakdown -->
  <div class="mt-2">
    <ScoreBreakdown breakdown={candidate.scoreBreakdown} score={candidate.score ?? 0} />
  </div>
</div>
