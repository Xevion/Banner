<script lang="ts">
import type { CodeDescription } from "$lib/bindings";
import { getFiltersContext } from "$lib/stores/search-filters.svelte";
import DaysOfWeekPicker from "./DaysOfWeekPicker.svelte";
import FilterPopover from "./FilterPopover.svelte";
import PartOfTermPicker from "./PartOfTermPicker.svelte";
import TimeRangeInput from "./TimeRangeInput.svelte";

let {
  referenceData,
}: {
  referenceData: {
    partsOfTerm: CodeDescription[];
  };
} = $props();

const filters = getFiltersContext();
const hasActiveFilters = $derived(
  filters.days.length > 0 ||
    filters.timeStart !== null ||
    filters.timeEnd !== null ||
    filters.partOfTerm.length > 0
);
</script>

<FilterPopover label="Schedule" active={hasActiveFilters}>
  {#snippet content()}
    <DaysOfWeekPicker bind:days={filters.days} />

    <div class="h-px bg-border"></div>

    <TimeRangeInput bind:timeStart={filters.timeStart} bind:timeEnd={filters.timeEnd} />

    {#if referenceData.partsOfTerm.length > 0}
      <div class="h-px bg-border"></div>

      <PartOfTermPicker bind:partOfTerm={filters.partOfTerm} partsOfTerm={referenceData.partsOfTerm} />
    {/if}
  {/snippet}
</FilterPopover>
