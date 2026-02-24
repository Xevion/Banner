<script lang="ts">
import type { CodeDescription } from "$lib/bindings";
import { groupAttributes } from "$lib/filters";
import { ATTRIBUTE_GROUPS, FORMAT_BUTTONS } from "$lib/labels";
import { getFiltersContext } from "$lib/stores/search-filters.svelte";
import { ChevronDown } from "@lucide/svelte";
import AttributeSection from "./AttributeSection.svelte";
import AvailabilityFilter from "./AvailabilityFilter.svelte";
import BottomSheet from "./BottomSheet.svelte";
import CompoundFilterButton from "./CompoundFilterButton.svelte";
import DaysOfWeekPicker from "./DaysOfWeekPicker.svelte";
import PartOfTermPicker from "./PartOfTermPicker.svelte";
import RangeSlider from "./RangeSlider.svelte";
import TimeRangeInput from "./TimeRangeInput.svelte";

type AttributeReferenceData = Record<
  "instructionalMethods" | "campuses" | "partsOfTerm" | "attributes",
  CodeDescription[]
>;

let {
  open = $bindable(false),
  referenceData,
  ranges,
}: {
  open: boolean;
  referenceData: AttributeReferenceData;
  ranges: {
    courseNumber: { min: number; max: number };
    creditHours: { min: number; max: number };
    waitCount: { max: number };
  };
} = $props();

const filters = getFiltersContext();

let expandedSection = $state<string | null>(null);

function toggleSection(id: string) {
  expandedSection = expandedSection === id ? null : id;
}

const grouped = $derived(groupAttributes(referenceData.attributes, ATTRIBUTE_GROUPS));

// Format course number pips as "0", "1k", "2k", etc.
function formatCourseNumberPip(v: number): string {
  if (v === 0) return "0";
  return `${v / 1000}k`;
}

// Active state for each filter section
const statusActive = $derived(filters.openOnly || filters.waitCountMax !== null);
const formatActive = $derived(filters.instructionalMethod.length > 0);
const scheduleActive = $derived(
  filters.days.length > 0 ||
    filters.timeStart !== null ||
    filters.timeEnd !== null ||
    filters.partOfTerm.length > 0
);
const catalogActive = $derived(filters.campus.length > 0 || filters.attributes.length > 0);
const moreActive = $derived(
  filters.creditHourMin !== null ||
    filters.creditHourMax !== null ||
    !!filters.instructor ||
    filters.courseNumberLow !== null ||
    filters.courseNumberHigh !== null
);

// Show chevron when section is expanded OR when filter is not active
const showStatusChevron = $derived(expandedSection === "status" || !statusActive);
const showFormatChevron = $derived(expandedSection === "format" || !formatActive);
const showScheduleChevron = $derived(expandedSection === "schedule" || !scheduleActive);
const showCatalogChevron = $derived(expandedSection === "catalog" || !catalogActive);
const showMoreChevron = $derived(expandedSection === "more" || !moreActive);
</script>

<BottomSheet bind:open>
  <div class="flex flex-col">
    <!-- Status section -->
    <button
      onclick={() => toggleSection("status")}
      class="flex items-center justify-between px-4 py-3 text-sm font-medium text-foreground"
    >
      <span>Status</span>
      <div class="relative size-4 flex items-center justify-center">
        <ChevronDown
          class="size-4 text-muted-foreground absolute transition-all duration-150 {showStatusChevron
            ? 'opacity-100 scale-100'
            : 'opacity-0 scale-75'} {expandedSection === 'status' ? 'rotate-180' : ''}"
        />
        <span
          class="size-1.5 rounded-full bg-primary absolute transition-all duration-150 {showStatusChevron
            ? 'opacity-0 scale-75'
            : 'opacity-100 scale-100'}"
        ></span>
      </div>
    </button>
    {#if expandedSection === "status"}
      <div class="px-4 pb-3 flex flex-col gap-3">
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

        {#if ranges.waitCount.max > 0}
          <RangeSlider
            min={0}
            max={ranges.waitCount.max}
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
      </div>
    {/if}
    <div class="h-px bg-border mx-4"></div>

    <!-- Format section (was part of Attributes) -->
    <button
      onclick={() => toggleSection("format")}
      class="flex items-center justify-between px-4 py-3 text-sm font-medium text-foreground"
    >
      <span>Format</span>
      <div class="relative size-4 flex items-center justify-center">
        <ChevronDown
          class="size-4 text-muted-foreground absolute transition-all duration-150 {showFormatChevron
            ? 'opacity-100 scale-100'
            : 'opacity-0 scale-75'} {expandedSection === 'format' ? 'rotate-180' : ''}"
        />
        <span
          class="size-1.5 rounded-full bg-primary absolute transition-all duration-150 {showFormatChevron
            ? 'opacity-0 scale-75'
            : 'opacity-100 scale-100'}"
        ></span>
      </div>
    </button>
    {#if expandedSection === "format"}
      <div class="px-4 pb-3 flex flex-col gap-1.5">
        {#each FORMAT_BUTTONS as btn (btn.label)}
          <CompoundFilterButton
            label={btn.label}
            codes={btn.codes}
            variants={btn.variants}
            bind:selected={filters.instructionalMethod}
          />
        {/each}
      </div>
    {/if}
    <div class="h-px bg-border mx-4"></div>

    <!-- Schedule section (now includes Part of Term) -->
    <button
      onclick={() => toggleSection("schedule")}
      class="flex items-center justify-between px-4 py-3 text-sm font-medium text-foreground"
    >
      <span>Schedule</span>
      <div class="relative size-4 flex items-center justify-center">
        <ChevronDown
          class="size-4 text-muted-foreground absolute transition-all duration-150 {showScheduleChevron
            ? 'opacity-100 scale-100'
            : 'opacity-0 scale-75'} {expandedSection === 'schedule' ? 'rotate-180' : ''}"
        />
        <span
          class="size-1.5 rounded-full bg-primary absolute transition-all duration-150 {showScheduleChevron
            ? 'opacity-0 scale-75'
            : 'opacity-100 scale-100'}"
        ></span>
      </div>
    </button>
    {#if expandedSection === "schedule"}
      <div class="px-4 pb-3 flex flex-col gap-3">
        <DaysOfWeekPicker bind:days={filters.days} mobile />

        <div class="h-px bg-border"></div>

        <TimeRangeInput bind:timeStart={filters.timeStart} bind:timeEnd={filters.timeEnd} />

        {#if referenceData.partsOfTerm.length > 0}
          <div class="h-px bg-border"></div>

          <PartOfTermPicker bind:partOfTerm={filters.partOfTerm} partsOfTerm={referenceData.partsOfTerm} mobile />
        {/if}
      </div>
    {/if}
    <div class="h-px bg-border mx-4"></div>

    <!-- Catalog section (Availability + Course Attributes) -->
    <button
      onclick={() => toggleSection("catalog")}
      class="flex items-center justify-between px-4 py-3 text-sm font-medium text-foreground"
    >
      <span>Catalog</span>
      <div class="relative size-4 flex items-center justify-center">
        <ChevronDown
          class="size-4 text-muted-foreground absolute transition-all duration-150 {showCatalogChevron
            ? 'opacity-100 scale-100'
            : 'opacity-0 scale-75'} {expandedSection === 'catalog' ? 'rotate-180' : ''}"
        />
        <span
          class="size-1.5 rounded-full bg-primary absolute transition-all duration-150 {showCatalogChevron
            ? 'opacity-0 scale-75'
            : 'opacity-100 scale-100'}"
        ></span>
      </div>
    </button>
    {#if expandedSection === "catalog"}
      <div class="px-4 pb-3 flex flex-col gap-3">
        <AvailabilityFilter bind:campus={filters.campus} />

        {#if grouped.level.length > 0}
          <div class="h-px bg-border"></div>
          <AttributeSection title="Course Level" items={grouped.level} bind:selected={filters.attributes} />
        {/if}

        {#if grouped.core.length > 0}
          <div class="h-px bg-border"></div>
          <AttributeSection title="Core Curriculum" items={grouped.core} bind:selected={filters.attributes} />
        {/if}

        {#if grouped.special.length > 0}
          <div class="h-px bg-border"></div>
          <AttributeSection title="Designations" items={grouped.special} bind:selected={filters.attributes} />
        {/if}
      </div>
    {/if}
    <div class="h-px bg-border mx-4"></div>

    <!-- More section -->
    <button
      onclick={() => toggleSection("more")}
      class="flex items-center justify-between px-4 py-3 text-sm font-medium text-foreground"
    >
      <span>More</span>
      <div class="relative size-4 flex items-center justify-center">
        <ChevronDown
          class="size-4 text-muted-foreground absolute transition-all duration-150 {showMoreChevron
            ? 'opacity-100 scale-100'
            : 'opacity-0 scale-75'} {expandedSection === 'more' ? 'rotate-180' : ''}"
        />
        <span
          class="size-1.5 rounded-full bg-primary absolute transition-all duration-150 {showMoreChevron
            ? 'opacity-0 scale-75'
            : 'opacity-100 scale-100'}"
        ></span>
      </div>
    </button>
    {#if expandedSection === "more"}
      <div class="px-4 pb-3 flex flex-col gap-3">
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
          <label
            for="mobile-instructor-input"
            class="text-xs font-medium text-muted-foreground select-none"
          >
            Instructor
          </label>
          <input
            id="mobile-instructor-input"
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
      </div>
    {/if}
  </div>
</BottomSheet>
