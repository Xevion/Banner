<script lang="ts">
import type { CodeDescription } from "$lib/bindings";
import type { Subject, Term } from "$lib/api";
import { getFiltersContext } from "$lib/stores/search-filters.svelte";
import { SlidersHorizontal } from "@lucide/svelte";
import CatalogPopover from "./CatalogPopover.svelte";
import FormatPopover from "./FormatPopover.svelte";
import MobileFilterSheet from "./MobileFilterSheet.svelte";
import MorePopover from "./MorePopover.svelte";
import SchedulePopover from "./SchedulePopover.svelte";
import StatusPopover from "./StatusPopover.svelte";
import SubjectCombobox from "./SubjectCombobox.svelte";
import TermCombobox from "./TermCombobox.svelte";

let {
  terms,
  subjects,
  selectedTerm = $bindable(),
  referenceData,
  ranges,
}: {
  terms: Term[];
  subjects: Subject[];
  selectedTerm: string;
  referenceData: {
    instructionalMethods: CodeDescription[];
    campuses: CodeDescription[];
    partsOfTerm: CodeDescription[];
    attributes: CodeDescription[];
  };
  ranges: {
    courseNumber: { min: number; max: number };
    creditHours: { min: number; max: number };
    waitCount: { max: number };
  };
} = $props();

const filters = getFiltersContext();

// Mobile bottom sheet state
let filterSheetOpen = $state(false);

// Active filter count is now derived from the filters instance
let activeFilterCount = $derived(filters.activeCount);
</script>

<!-- Mobile row 1: Term + Subject side by side -->
<div class="flex gap-2 md:hidden">
  <div class="flex-1 min-w-0">
    <TermCombobox {terms} bind:value={selectedTerm} />
  </div>
  <div class="flex-1 min-w-0">
    <SubjectCombobox {subjects} bind:value={filters.subject} />
  </div>
</div>

<!-- Mobile row 2: Search + Filters button -->
<div class="flex gap-2 md:hidden">
  <input
    type="text"
    placeholder="Search courses..."
    aria-label="Search courses"
    bind:value={filters.query}
    class="h-9 border border-border bg-card text-foreground rounded-md px-3 text-sm flex-1 min-w-0
           focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background
           transition-colors"
  />
  <button
    onclick={() => (filterSheetOpen = true)}
    class="inline-flex items-center gap-1.5 rounded-md border h-9 px-3 text-sm font-medium transition-colors cursor-pointer select-none shrink-0
           {activeFilterCount > 0
      ? 'border-primary/50 bg-primary/10 text-primary'
      : 'border-border bg-background text-muted-foreground hover:bg-accent hover:text-accent-foreground'}"
  >
    <SlidersHorizontal class="size-3.5" />
    Filters
    {#if activeFilterCount > 0}
      <span
        class="inline-flex items-center justify-center size-4 rounded-full bg-primary text-primary-foreground text-[10px] font-semibold"
        >{activeFilterCount}</span
      >
    {/if}
  </button>
</div>

<!-- Desktop row 1: Term + Subject + Search (unchanged) -->
<div class="hidden md:flex flex-wrap gap-3 items-start">
  <TermCombobox {terms} bind:value={selectedTerm} />
  <SubjectCombobox {subjects} bind:value={filters.subject} />
  <input
    type="text"
    placeholder="Search courses..."
    aria-label="Search courses"
    bind:value={filters.query}
    class="h-9 border border-border bg-card text-foreground rounded-md px-3 text-sm flex-1 min-w-[200px]
           focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background
           transition-colors"
  />
</div>

<!-- Desktop row 2: Category filter popovers -->
<div class="hidden md:flex flex-wrap gap-2 items-center">
  <StatusPopover waitCountMaxRange={ranges.waitCount.max} />
  <FormatPopover />
  <SchedulePopover
    referenceData={{ partsOfTerm: referenceData.partsOfTerm }}
  />
  <CatalogPopover
    referenceData={{ attributes: referenceData.attributes }}
  />
  <MorePopover
    ranges={{ courseNumber: ranges.courseNumber, creditHours: ranges.creditHours }}
  />
</div>

<!-- Mobile: Filter bottom sheet -->
<MobileFilterSheet
  bind:open={filterSheetOpen}
  {referenceData}
  {ranges}
/>
