<script lang="ts">
import { getFiltersContext } from "$lib/stores/search-filters.svelte";
import FilterPopover from "./FilterPopover.svelte";
import InstructorAutocomplete from "./InstructorAutocomplete.svelte";
import RangeSlider from "./RangeSlider.svelte";

let {
  ranges,
  selectedTerm,
}: {
  ranges: { courseNumber: { min: number; max: number }; creditHours: { min: number; max: number } };
  selectedTerm: string;
} = $props();

const filters = getFiltersContext();
const hasActiveFilters = $derived(
  filters.creditHourMin !== null ||
    filters.creditHourMax !== null ||
    filters.instructor.length > 0 ||
    filters.courseNumberLow !== null ||
    filters.courseNumberHigh !== null
);

// Format course number pips as "0", "1k", "2k", etc.
function formatCourseNumberPip(v: number): string {
  if (v === 0) return "0";
  return `${v / 1000}k`;
}
</script>

<FilterPopover label="More" active={hasActiveFilters}>
  {#snippet content()}
    <RangeSlider
      min={ranges.creditHours.min}
      max={ranges.creditHours.max}
      step={1}
      bind:valueLow={filters.creditHourMin}
      bind:valueHigh={filters.creditHourMax}
      label="Credit hours"
      pips
      all="label"
    />

    <div class="h-px bg-border"></div>

    <InstructorAutocomplete {selectedTerm} />

    <div class="h-px bg-border"></div>

    <RangeSlider
      min={ranges.courseNumber.min}
      max={ranges.courseNumber.max}
      step={100}
      bind:valueLow={filters.courseNumberLow}
      bind:valueHigh={filters.courseNumberHigh}
      label="Course number"
      formatPip={formatCourseNumberPip}
      pips
      pipstep={10}
      all="label"
    />
  {/snippet}
</FilterPopover>
