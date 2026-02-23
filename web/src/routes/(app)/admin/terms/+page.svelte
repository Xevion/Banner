<script lang="ts">
import { client } from "$lib/api";
import type { DbTerm } from "$lib/bindings";
import SimpleTooltip from "$lib/components/SimpleTooltip.svelte";
import { formatAbsoluteDate, formatRelativeDate } from "$lib/date";
import { RefreshCw } from "@lucide/svelte";
import type { PageProps } from "./$types";

let { data }: PageProps = $props();
let terms = $state<DbTerm[]>(data.terms);
let error = $state<string | null>(data.error);

// Track in-flight toggle per term code
let togglingCodes = $state(new Set<string>());

// Sync state
let syncing = $state(false);
let syncMessage = $state<string | null>(null);

async function toggleTerm(term: DbTerm) {
  togglingCodes.add(term.code);
  togglingCodes = togglingCodes; // trigger reactivity

  const result = term.scrapeEnabled
    ? await client.disableTerm(term.code)
    : await client.enableTerm(term.code);

  togglingCodes.delete(term.code);
  togglingCodes = togglingCodes;

  if (result.isErr) {
    error = result.error.message;
    return;
  }

  if (result.value.term) {
    const updated = result.value.term;
    terms = terms.map((t) => (t.code === updated.code ? updated : t));
  }
}

function getTermStatus(term: DbTerm): "past" | "current" | "future" {
  const now = new Date();
  const month = now.getMonth() + 1;
  const day = now.getDate();
  const year = now.getFullYear();
  const mmdd = month * 100 + day;

  let currentYear: number;
  let currentSeason: string;

  if (mmdd >= 114 && mmdd <= 501) {
    currentSeason = "Spring";
    currentYear = year;
  } else if (mmdd >= 502 && mmdd <= 524) {
    // Gap between Spring and Summer — treat Summer as current
    currentSeason = "Summer";
    currentYear = year;
  } else if (mmdd >= 525 && mmdd <= 815) {
    currentSeason = "Summer";
    currentYear = year;
  } else if (mmdd >= 816 && mmdd <= 817) {
    // Gap between Summer and Fall — treat Fall as current
    currentSeason = "Fall";
    currentYear = year;
  } else if (mmdd >= 818 && mmdd <= 1210) {
    currentSeason = "Fall";
    currentYear = year;
  } else {
    // Dec 11–31 or Jan 1–13: between Fall and Spring — treat Spring as current
    currentSeason = "Spring";
    currentYear = mmdd >= 1211 ? year + 1 : year;
  }

  const seasonStart = (s: string) => (s === "Spring" ? 1 : s === "Summer" ? 5 : 8);
  const currentIndex = currentYear * 12 + seasonStart(currentSeason);
  const termIndex = term.year * 12 + seasonStart(term.season);

  if (termIndex < currentIndex) return "past";
  if (termIndex > currentIndex) return "future";
  return "current";
}

async function syncTerms() {
  syncing = true;
  syncMessage = null;

  const result = await client.syncTerms();

  syncing = false;

  if (result.isErr) {
    error = result.error.message;
    return;
  }

  syncMessage = `Synced: ${result.value.inserted} inserted, ${result.value.updated} updated`;

  // Refetch the term list
  const refreshResult = await client.getAdminTerms();
  if (refreshResult.isOk) {
    terms = refreshResult.value.terms;
  }
}
</script>

<svelte:head>
  <title>Terms | Banner</title>
</svelte:head>

<div class="flex flex-col gap-y-4">
  <div class="flex items-center justify-between">
    <h1 class="text-lg font-semibold text-foreground">Terms</h1>
    <button
      onclick={syncTerms}
      disabled={syncing}
      class="inline-flex items-center gap-1.5 rounded-md bg-muted px-3 py-1.5 text-sm font-medium
        text-foreground transition-colors hover:bg-muted/80 disabled:opacity-50 disabled:cursor-not-allowed cursor-pointer"
    >
      <RefreshCw class="size-3.5 {syncing ? 'animate-spin' : ''}" />
      Sync from Banner
    </button>
  </div>

  {#if syncMessage}
    <p class="text-sm text-green-600 dark:text-green-400">{syncMessage}</p>
  {/if}

  {#if error}
    <p class="text-sm text-destructive">{error}</p>
  {/if}

  <div class="bg-card border-border rounded-lg border overflow-x-auto">
    <table class="w-full text-sm">
      <thead>
        <tr class="border-b border-border text-left text-muted-foreground">
          <th class="px-4 py-2.5 font-medium">Description</th>
          <th class="px-4 py-2.5 font-medium">Code</th>
          <th class="px-4 py-2.5 font-medium hidden sm:table-cell">Season</th>
          <th class="px-4 py-2.5 font-medium hidden md:table-cell">Status</th>
          <th class="px-4 py-2.5 font-medium hidden lg:table-cell">Last Scraped</th>
          <th class="px-4 py-2.5 font-medium text-center">Scraping</th>
        </tr>
      </thead>
      <tbody>
        {#each terms as term (term.code)}
          {@const toggling = togglingCodes.has(term.code)}
          {@const status = getTermStatus(term)}
          <tr class="border-b border-border last:border-b-0 hover:bg-muted/30 transition-colors">
            <td class="px-4 py-2.5 font-medium text-foreground">{term.description}</td>
            <td class="px-4 py-2.5 text-muted-foreground font-mono text-xs">{term.code}</td>
            <td class="px-4 py-2.5 text-muted-foreground hidden sm:table-cell">{term.season}</td>
            <td class="px-4 py-2.5 hidden md:table-cell">
              {#if status === 'past'}
                <span class="rounded-full bg-amber-100 px-2 py-0.5 text-xs font-medium text-amber-800 dark:bg-amber-900 dark:text-amber-200">
                  Archived
                </span>
              {:else if status === 'current'}
                <span class="rounded-full bg-green-100 px-2 py-0.5 text-xs font-medium text-green-800 dark:bg-green-900 dark:text-green-200">
                  Current
                </span>
              {:else}
                <span class="rounded-full bg-blue-100 px-2 py-0.5 text-xs font-medium text-blue-800 dark:bg-blue-900 dark:text-blue-200">
                  Upcoming
                </span>
              {/if}
            </td>
            <td class="px-4 py-2.5 text-muted-foreground hidden lg:table-cell">
              {#if term.lastScrapedAt}
                <SimpleTooltip text={formatAbsoluteDate(term.lastScrapedAt)} side="top" passthrough>
                  <span>{formatRelativeDate(term.lastScrapedAt)}</span>
                </SimpleTooltip>
              {:else}
                <span class="text-muted-foreground/60">Never</span>
              {/if}
            </td>
            <td class="px-4 py-2.5 text-center">
              <button
                onclick={() => toggleTerm(term)}
                disabled={toggling}
                class="relative inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full
                  transition-colors disabled:opacity-50 disabled:cursor-not-allowed
                  {term.scrapeEnabled ? 'bg-green-600' : 'bg-muted-foreground/30'}"
                role="switch"
                aria-checked={term.scrapeEnabled}
                aria-label="Toggle scraping for {term.description}"
              >
                <span
                  class="pointer-events-none inline-block size-3.5 rounded-full bg-white shadow-sm
                    transition-transform {term.scrapeEnabled ? 'translate-x-[18px]' : 'translate-x-[3px]'}"
                ></span>
              </button>
            </td>
          </tr>
        {:else}
          <tr>
            <td colspan="6" class="px-4 py-8 text-center text-muted-foreground">
              No terms found. Click "Sync from Banner" to fetch terms.
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
</div>
