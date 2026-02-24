<script lang="ts">
import { client } from "$lib/api";
import type { ServiceStatus } from "$lib/bindings";
import { formatNumber } from "$lib/utils";
import { RefreshCw } from "@lucide/svelte";
import type { PageProps } from "./$types";

let { data }: PageProps = $props();
let status = $derived(data.status);
let error = $derived(data.error);

let syncingBlueBook = $state(false);
let blueBookMessage = $state<string | null>(null);

async function syncBlueBook() {
  syncingBlueBook = true;
  blueBookMessage = null;

  const result = await client.syncBlueBook();

  syncingBlueBook = false;

  if (result.isErr) {
    blueBookMessage = result.error.message;
    return;
  }

  blueBookMessage = result.value.message;
}

const STATUS_COLORS: Record<ServiceStatus, string> = {
  active: "var(--status-green)",
  connected: "var(--status-green)",
  starting: "var(--status-orange)",
  disabled: "var(--status-gray)",
  error: "var(--status-red)",
};

function formatStatus(s: string): string {
  return s.charAt(0).toUpperCase() + s.slice(1);
}

function toTitleCase(s: string): string {
  return s.replace(/\b\w/g, (c) => c.toUpperCase());
}
</script>

<svelte:head>
  <title>Dashboard | Banner</title>
</svelte:head>

<h1 class="mb-4 text-lg font-semibold text-foreground">Dashboard</h1>

{#if error}
  <p class="text-destructive">{error}</p>
{:else if !status}
  <p class="text-muted-foreground">Loading...</p>
{:else}
  <div class="grid grid-cols-2 gap-4 lg:grid-cols-4">
    <div class="bg-card border-border rounded-lg border p-4">
      <p class="text-muted-foreground text-sm select-none">Users</p>
      <p class="text-3xl font-bold select-none">{formatNumber(status.userCount)}</p>
    </div>
    <div class="bg-card border-border rounded-lg border p-4">
      <p class="text-muted-foreground text-sm select-none">Active Sessions</p>
      <p class="text-3xl font-bold select-none">{formatNumber(status.sessionCount)}</p>
    </div>
    <div class="bg-card border-border rounded-lg border p-4">
      <p class="text-muted-foreground text-sm select-none">Courses</p>
      <p class="text-3xl font-bold select-none">{formatNumber(status.courseCount)}</p>
    </div>
    <div class="bg-card border-border rounded-lg border p-4">
      <p class="text-muted-foreground text-sm select-none">Scrape Jobs</p>
      <p class="text-3xl font-bold select-none">{formatNumber(status.scrapeJobCount)}</p>
    </div>
  </div>

  <h2 class="mt-6 mb-3 text-sm font-semibold text-foreground">Services</h2>
  <div class="bg-card border-border rounded-lg border">
    {#each status.services as service (service.name)}
      {@const color = STATUS_COLORS[service.status] ?? "var(--status-gray)"}
      <div class="border-border flex items-center justify-between border-b px-4 py-3 last:border-b-0">
        <span class="font-medium select-none">{toTitleCase(service.name)}</span>
        <span
          class="rounded-full px-2.5 py-0.5 text-xs font-medium select-none"
          style="background-color: color-mix(in oklch, {color} 15%, transparent); color: {color}"
        >
          {formatStatus(service.status)}
        </span>
      </div>
    {/each}
  </div>

  <h2 class="mt-6 mb-3 text-sm font-semibold text-foreground">Quick Actions</h2>
  <div class="bg-card border-border rounded-lg border p-4 flex items-center justify-between">
    <div>
      <p class="font-medium text-foreground">BlueBook Sync</p>
      <p class="text-sm text-muted-foreground">Trigger a full BlueBook course evaluation scrape</p>
    </div>
    <div class="flex items-center gap-3">
      {#if blueBookMessage}
        <span class="text-sm text-green-600 dark:text-green-400">{blueBookMessage}</span>
      {/if}
      <button
        onclick={syncBlueBook}
        disabled={syncingBlueBook}
        class="inline-flex items-center gap-1.5 rounded-md bg-muted px-3 py-1.5 text-sm font-medium
          text-foreground transition-colors hover:bg-muted/80 disabled:opacity-50 disabled:cursor-not-allowed cursor-pointer"
      >
        <RefreshCw class="size-3.5 {syncingBlueBook ? 'animate-spin' : ''}" />
        {syncingBlueBook ? "Syncing..." : "Sync BlueBook"}
      </button>
    </div>
  </div>
{/if}
