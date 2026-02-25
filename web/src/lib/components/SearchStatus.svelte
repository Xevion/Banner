<script lang="ts">
import SimpleTooltip from "$lib/components/SimpleTooltip.svelte";
import { relativeTime } from "$lib/time";
import { formatNumber } from "$lib/utils";
import { onMount } from "svelte";
import { fade } from "svelte/transition";

export interface SearchMeta {
  totalCount: number;
  durationMs: number;
  timestamp: Date;
}

let { meta, loading = false }: { meta: SearchMeta | null; loading?: boolean } = $props();

let now = $state(new Date());

let formattedTime = $derived(
  meta
    ? meta.timestamp.toLocaleTimeString(undefined, {
        hour: "2-digit",
        minute: "2-digit",
        second: "2-digit",
      })
    : ""
);

let relativeTimeResult = $derived(meta ? relativeTime(meta.timestamp, now) : null);
let relativeTimeText = $derived(relativeTimeResult?.text ?? "");

let countLabel = $derived(meta ? formatNumber(meta.totalCount) : "");
let resultNoun = $derived(meta ? (meta.totalCount !== 1 ? "results" : "result") : "");
let durationLabel = $derived(meta ? `${Math.round(meta.durationMs)}ms` : "");

let tooltipText = $derived(meta ? `${relativeTimeText} &middot; ${formattedTime}` : "");

onMount(() => {
  let nowTimeoutId: ReturnType<typeof setTimeout> | null = null;

  function scheduleNowTick() {
    const delay = relativeTimeResult?.nextUpdateMs ?? 1000;
    nowTimeoutId = setTimeout(() => {
      now = new Date();
      scheduleNowTick();
    }, delay);
  }
  scheduleNowTick();

  return () => {
    if (nowTimeoutId) clearTimeout(nowTimeoutId);
  };
});
</script>

{#if meta}
    <SimpleTooltip
        text={tooltipText}
        contentClass="whitespace-nowrap text-[12px]/1 px-2"
        sideOffset={0}
    >
        <span
            class="pl-1 text-xs transition-opacity duration-200 select-none {loading
                ? 'opacity-40'
                : ''}"
            in:fade={{ duration: 300 }}
        >
            <span class="text-muted-foreground/70">{countLabel}</span>
            <span class="text-muted-foreground/35">{resultNoun} in</span>
            <span class="text-muted-foreground/70">{durationLabel}</span>
        </span>
    </SimpleTooltip>
{:else}
    <!-- Invisible placeholder to maintain layout height -->
    <span class="pl-1 text-xs opacity-0 pointer-events-none" aria-hidden="true"
        >&nbsp;</span
    >
{/if}
