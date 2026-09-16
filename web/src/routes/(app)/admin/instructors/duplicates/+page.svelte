<script lang="ts">
import { client } from "$lib/api";
import type { DuplicatePair, DuplicateSide, DuplicateTier } from "$lib/bindings";
import ActionResultBanner from "$lib/components/ActionResultBanner.svelte";
import { formatInstructorName } from "$lib/course";
import { ArrowRight, LoaderCircle, Merge } from "@lucide/svelte";
import { untrack } from "svelte";
import type { PageProps } from "./$types";

let { data }: PageProps = $props();

let pairs = $state<DuplicatePair[]>(untrack(() => data.pairs));
let error = $state<string | null>(untrack(() => data.error));
let actionLoading = $state<string | null>(null);
let mergeAllLoading = $state(false);
let result = $state<{ message: string; isError: boolean } | null>(null);

const TIER_LABEL: Record<DuplicateTier, string> = {
  sameAccount: "Same account",
  missingEmail: "No email on one side",
  differentAccount: "Different accounts",
};

const TIER_HINT: Record<DuplicateTier, string> = {
  sameAccount: "One UTSA account reached through both its student and staff domain.",
  missingEmail:
    "Only the name ties these together, so a same-name colleague could be merged by mistake.",
  differentAccount: "Distinct accounts. Usually two different people who share a name.",
};

const TIER_CLASSES: Record<DuplicateTier, string> = {
  sameAccount: "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400",
  missingEmail: "bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400",
  differentAccount: "bg-muted text-muted-foreground",
};

const safeCount = $derived(pairs.filter((p) => p.tier === "sameAccount").length);

async function refresh() {
  const res = await client.getInstructorDuplicates();
  if (res.isOk) {
    pairs = res.value.pairs;
    error = null;
  } else {
    error = res.error.message;
  }
}

async function handleMerge(pair: DuplicatePair) {
  const key = `${pair.survivor.id}-${pair.loser.id}`;
  actionLoading = key;
  const res = await client.mergeInstructors(pair.survivor.id, pair.loser.id);
  actionLoading = null;

  if (res.isOk) {
    result = {
      message: `Merged ${formatInstructorName(pair.loser.displayName)} into the record with ${pair.survivor.courseCount} sections.`,
      isError: false,
    };
    await refresh();
  } else {
    result = { message: res.error.message, isError: true };
  }
}

async function handleMergeAll() {
  mergeAllLoading = true;
  const res = await client.mergeAllDuplicates();
  mergeAllLoading = false;

  if (res.isOk) {
    result = {
      message: `Merged ${res.value.merged} record${res.value.merged === 1 ? "" : "s"}; ${res.value.skipped} left for review.`,
      isError: false,
    };
    await refresh();
  } else {
    result = { message: res.error.message, isError: true };
  }
}
</script>

<svelte:head><title>Duplicate instructors</title></svelte:head>

<div class="mx-auto max-w-5xl px-4 py-6">
  <div class="mb-4 flex items-start justify-between gap-4">
    <div>
      <h1 class="text-xl font-semibold text-foreground">Duplicate instructors</h1>
      <p class="mt-1 text-sm text-muted-foreground">
        UTSA issues a student and a staff address, so one person can hold several records.
        Merging moves every section, link and candidate onto the surviving record.
      </p>
    </div>
    {#if safeCount > 0}
      <button
        onclick={() => void handleMergeAll()}
        disabled={mergeAllLoading}
        class="inline-flex shrink-0 items-center gap-1.5 rounded-md bg-primary px-3 py-2 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-50 cursor-pointer"
      >
        {#if mergeAllLoading}
          <LoaderCircle size={14} class="animate-spin" />
        {:else}
          <Merge size={14} />
        {/if}
        Merge {safeCount} same-account
      </button>
    {/if}
  </div>

  <ActionResultBanner {result} onDismiss={() => (result = null)} />

  {#if error}
    <div class="rounded-md bg-destructive/10 px-3 py-2 text-sm text-destructive">{error}</div>
  {:else if pairs.length === 0}
    <div class="rounded-md border border-border bg-card px-4 py-8 text-center text-sm text-muted-foreground">
      No duplicate instructor records found.
    </div>
  {:else}
    <div class="space-y-3">
      {#each pairs as pair (`${pair.survivor.id}-${pair.loser.id}`)}
        {@const key = `${pair.survivor.id}-${pair.loser.id}`}
        <div class="rounded-md border border-border bg-card p-3">
          <div class="mb-2 flex items-center justify-between gap-2">
            <div class="flex items-center gap-2">
              <span class="font-medium text-foreground">
                {formatInstructorName(pair.survivor.displayName)}
              </span>
              <span class="rounded px-1.5 py-0.5 text-[10px] font-medium {TIER_CLASSES[pair.tier]}">
                {TIER_LABEL[pair.tier]}
              </span>
              {#if pair.subjectsOverlap}
                <span class="rounded bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground">
                  Shared subjects
                </span>
              {/if}
            </div>
            <button
              onclick={() => void handleMerge(pair)}
              disabled={actionLoading !== null}
              class="inline-flex items-center gap-1 rounded px-2 py-1 text-xs font-medium text-primary transition-colors hover:bg-muted disabled:opacity-50 cursor-pointer"
            >
              {#if actionLoading === key}
                <LoaderCircle size={12} class="animate-spin" />
              {:else}
                <Merge size={12} />
              {/if}
              Merge
            </button>
          </div>

          <p class="mb-2 text-xs text-muted-foreground">{TIER_HINT[pair.tier]}</p>

          <div class="grid gap-2 sm:grid-cols-[1fr_auto_1fr] sm:items-center">
            {@render side(pair.loser, false)}
            <ArrowRight size={16} class="mx-auto hidden text-muted-foreground sm:block" />
            {@render side(pair.survivor, true)}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

{#snippet side(person: DuplicateSide, keeps: boolean)}
  <div
    class="rounded border p-2 text-xs {keeps
      ? 'border-l-4 border-l-green-500 border-border bg-green-500/5'
      : 'border-border bg-muted/30'}"
  >
    <div class="flex items-center gap-1.5">
      <span class="font-medium text-foreground">#{person.id}</span>
      <span class="text-[10px] text-muted-foreground">{keeps ? "kept" : "absorbed"}</span>
    </div>
    <div class="mt-0.5 break-all text-muted-foreground">{person.email ?? "no email"}</div>
    <div class="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-muted-foreground">
      <span>{person.courseCount} sections</span>
      <span>{person.matchStatus}</span>
      {#if person.rmpLegacyIds.length > 0}
        <span>{person.rmpLegacyIds.length} RMP link{person.rmpLegacyIds.length === 1 ? "" : "s"}</span>
      {/if}
    </div>
    {#if person.subjects.length > 0}
      <div class="mt-1 flex flex-wrap gap-1">
        {#each person.subjects.slice(0, 6) as subject (subject)}
          <span class="rounded bg-muted px-1 py-0.5 text-[10px] text-muted-foreground">{subject}</span>
        {/each}
      </div>
    {/if}
  </div>
{/snippet}
