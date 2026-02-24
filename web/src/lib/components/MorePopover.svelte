<script lang="ts">
import { getFiltersContext } from "$lib/stores/search-filters.svelte";
import FilterPopover from "./FilterPopover.svelte";
import RangeSlider from "./RangeSlider.svelte";

let {
  ranges,
}: {
  ranges: { courseNumber: { min: number; max: number }; creditHours: { min: number; max: number } };
} = $props();

const filters = getFiltersContext();
const hasActiveFilters = $derived(
  filters.creditHourMin !== null ||
    filters.creditHourMax !== null ||
    !!filters.instructor ||
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

    <div class="flex flex-col gap-1.5">
      <label for="instructor-input" class="text-xs font-medium text-muted-foreground select-none">
        Instructor
      </label>
      <input
        id="instructor-input"
        type="text"
        placeholder="Search by name..."
        autocomplete="off"
        value={filters.instructor ?? ""}
        oninput={(e) => {
          filters.instructor = e.currentTarget.value || null;
        }}
        class="h-8 border border-border bg-card text-foreground rounded-md px-2 text-sm
               focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background"
      />
    </div>

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
