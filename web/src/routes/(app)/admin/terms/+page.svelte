<script lang="ts">
import { client } from "$lib/api";
import type { DbTerm } from "$lib/bindings";
import { formatRelativeDate, formatAbsoluteDate } from "$lib/date";
import SimpleTooltip from "$lib/components/SimpleTooltip.svelte";
import { RefreshCw } from "@lucide/svelte";

let { data } = $props();
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
          <tr class="border-b border-border last:border-b-0 hover:bg-muted/30 transition-colors">
            <td class="px-4 py-2.5 font-medium text-foreground">{term.description}</td>
            <td class="px-4 py-2.5 text-muted-foreground font-mono text-xs">{term.code}</td>
            <td class="px-4 py-2.5 text-muted-foreground hidden sm:table-cell">{term.season}</td>
            <td class="px-4 py-2.5 hidden md:table-cell">
              {#if term.isArchived}
                <span class="rounded-full bg-amber-100 px-2 py-0.5 text-xs font-medium text-amber-800 dark:bg-amber-900 dark:text-amber-200">
                  Archived
                </span>
              {:else}
                <span class="rounded-full bg-green-100 px-2 py-0.5 text-xs font-medium text-green-800 dark:bg-green-900 dark:text-green-200">
                  Active
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
