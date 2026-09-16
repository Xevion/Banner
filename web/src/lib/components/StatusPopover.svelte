<script lang="ts">
import { getFiltersContext } from "$lib/stores/search-filters.svelte";
import FilterPopover from "./FilterPopover.svelte";
import RangeSlider from "./RangeSlider.svelte";

let {
  waitCountMaxRange,
}: {
  waitCountMaxRange: number;
} = $props();

const filters = getFiltersContext();
const hasActiveFilters = $derived(filters.openOnly || filters.waitCountMax !== null);
</script>

<FilterPopover label="Status" active={hasActiveFilters} width="min-w-64">
  {#snippet content()}
    <div class="flex flex-col gap-1.5">
      <span class="text-xs font-medium text-muted-foreground select-none">Availability</span>
      <button
        type="button"
        aria-pressed={filters.openOnly}
        class="inline-flex items-center justify-center rounded-full px-3 py-1 text-xs font-medium transition-colors cursor-pointer select-none
               {filters.openOnly
          ? 'bg-primary text-primary-foreground'
          : 'bg-muted text-muted-foreground hover:bg-muted/80'}"
        onclick={() => (filters.openOnly = !filters.openOnly)}
      >
        Open only
      </button>
    </div>

    <div class="h-px bg-border"></div>

    {#if waitCountMaxRange > 0}
      <RangeSlider
        min={0}
        max={waitCountMaxRange}
        step={5}
        bind:value={filters.waitCountMax}
        label="Max waitlist"
        dual={false}
        pips
        pipstep={2}
        formatValue={(v) => (v === 0 ? "Off" : String(v))}
      />
    {:else}
      <div class="flex flex-col gap-1.5">
        <span class="text-xs font-medium text-muted-foreground select-none">Max waitlist</span>
        <span class="text-xs text-muted-foreground select-none">No waitlisted courses</span>
      </div>
    {/if}
  {/snippet}
</FilterPopover>
