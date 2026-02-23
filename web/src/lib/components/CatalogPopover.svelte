<script lang="ts">
import type { CodeDescription } from "$lib/bindings";
import { groupAttributes } from "$lib/filters";
import { ATTRIBUTE_GROUPS } from "$lib/labels";
import { getFiltersContext } from "$lib/stores/search-filters.svelte";
import AttributeSection from "./AttributeSection.svelte";
import AvailabilityFilter from "./AvailabilityFilter.svelte";
import FilterPopover from "./FilterPopover.svelte";

let {
  referenceData,
}: {
  referenceData: {
    attributes: CodeDescription[];
  };
} = $props();

const filters = getFiltersContext();
const hasActiveFilters = $derived(filters.campus.length > 0 || filters.attributes.length > 0);

const grouped = $derived(groupAttributes(referenceData.attributes, ATTRIBUTE_GROUPS));
</script>

<FilterPopover label="Catalog" active={hasActiveFilters} width="w-[19rem] max-h-[28rem] overflow-y-auto">
  {#snippet content()}
    <AvailabilityFilter bind:campus={filters.campus} />

    <div class="h-px bg-border"></div>

    <AttributeSection title="Course Level" items={grouped.level} bind:selected={filters.attributes} />

    {#if grouped.level.length > 0}
      <div class="h-px bg-border"></div>
    {/if}

    <AttributeSection title="Core Curriculum" items={grouped.core} bind:selected={filters.attributes} />

    {#if grouped.special.length > 0}
      <div class="h-px bg-border"></div>

      <AttributeSection title="Designations" items={grouped.special} bind:selected={filters.attributes} />
    {/if}
  {/snippet}
</FilterPopover>
