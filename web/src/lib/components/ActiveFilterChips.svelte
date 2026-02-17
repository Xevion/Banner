<script lang="ts">
import type { DayOfWeek } from "$lib/bindings";
import { dayCode } from "$lib/days";
import { formatCompactTime } from "$lib/filters";
import {
  getCampusFilterLabel,
  getAttributeFilterLabel,
  getPartOfTermFilterLabel,
} from "$lib/labels";
import { type ScrollMetrics, maskGradient as computeMaskGradient } from "$lib/scroll-fade";
import type { SearchFilters } from "$lib/stores/search-filters.svelte";
import FilterChip from "$lib/components/FilterChip.svelte";
import SegmentedChip from "$lib/components/SegmentedChip.svelte";

let { filters }: { filters: SearchFilters } = $props();

// --- Chip formatting helpers ---

function formatDaysChip(d: string[]): string {
  return d.map((day) => dayCode(day as DayOfWeek)).join("");
}

function formatTimeChip(start: string | null, end: string | null): string {
  if (start && end) return `${formatCompactTime(start)} – ${formatCompactTime(end)}`;
  if (start) return `After ${formatCompactTime(start)}`;
  if (end) return `Before ${formatCompactTime(end)}`;
  return "";
}

function formatMultiChip(codes: string[], labelFn: (filterValue: string) => string): string {
  const first = labelFn(codes[0]);
  if (codes.length === 1) return first;
  return `${first} + ${codes.length - 1} more`;
}

// --- Instructional method grouping ---

interface FormatChipGroup {
  type: "InPerson" | "Online" | "Hybrid" | "Independent";
  codes: string[];
  label: string;
}

const VARIANT_LABELS: Record<string, string> = {
  "Online.Async": "Async",
  "Online.Sync": "Sync",
  "Online.Mixed": "Mix",
  "Hybrid.Half": "Half",
  "Hybrid.OneThird": "One Third",
  "Hybrid.TwoThirds": "Two Thirds",
};

function groupInstructionalMethods(methods: string[]): FormatChipGroup[] {
  const groups: FormatChipGroup[] = [];

  if (methods.includes("InPerson")) {
    groups.push({ type: "InPerson", codes: ["InPerson"], label: "In Person" });
  }
  if (methods.includes("Independent")) {
    groups.push({ type: "Independent", codes: ["Independent"], label: "Independent" });
  }

  const onlineCodes = methods.filter((m) => m.startsWith("Online."));
  if (onlineCodes.length > 0) {
    const variantLabels = onlineCodes.map((c) => VARIANT_LABELS[c] || c);
    groups.push({
      type: "Online",
      codes: onlineCodes,
      label: `Online: ${variantLabels.join(", ")}`,
    });
  }

  const hybridCodes = methods.filter((m) => m.startsWith("Hybrid."));
  if (hybridCodes.length > 0) {
    const variantLabels = hybridCodes.map((c) => VARIANT_LABELS[c] || c);
    groups.push({
      type: "Hybrid",
      codes: hybridCodes,
      label: `Hybrid: ${variantLabels.join(", ")}`,
    });
  }

  return groups;
}

function removeFormatGroup(group: FormatChipGroup) {
  filters.instructionalMethod = filters.instructionalMethod.filter((m) => !group.codes.includes(m));
}

let formatChipGroups = $derived(groupInstructionalMethods(filters.instructionalMethod));

function removeSubject(code: string) {
  filters.subject = filters.subject.filter((s) => s !== code);
}

// --- Scroll-based fade mask ---

let chipsContainer: HTMLDivElement | undefined = $state();
let scrollMetrics = $state<ScrollMetrics>({ scrollLeft: 0, scrollWidth: 0, clientWidth: 0 });

const maskGradient = $derived(computeMaskGradient(scrollMetrics));

function updateScrollMetrics() {
  if (!chipsContainer) return;
  scrollMetrics = {
    scrollLeft: chipsContainer.scrollLeft,
    scrollWidth: chipsContainer.scrollWidth,
    clientWidth: chipsContainer.clientWidth,
  };
}

$effect(() => {
  if (!chipsContainer) return;
  const el = chipsContainer;

  const ro = new ResizeObserver(updateScrollMetrics);
  ro.observe(el);

  el.addEventListener("scroll", updateScrollMetrics, { passive: true });
  updateScrollMetrics();

  return () => {
    ro.disconnect();
    el.removeEventListener("scroll", updateScrollMetrics);
  };
});
</script>

<!-- Active filter chips -->
<div
  bind:this={chipsContainer}
  class="flex items-center gap-1.5 flex-1 min-w-0
         flex-nowrap overflow-x-auto md:flex-wrap md:overflow-x-visible
         -mx-3 px-3 md:mx-0 md:px-0
         pb-1.5 scrollbar-none"
  style:mask-image={maskGradient}
  style:-webkit-mask-image={maskGradient}
>
  {#if filters.subject.length > 0}
    <SegmentedChip segments={filters.subject} onRemoveSegment={removeSubject} />
  {/if}
  {#if filters.openOnly}
    <FilterChip label="Open only" onRemove={() => (filters.openOnly = false)} />
  {/if}
  {#if filters.waitCountMax !== null}
    <FilterChip
      label="Waitlist ≤ {filters.waitCountMax}"
      onRemove={() => (filters.waitCountMax = null)}
    />
  {/if}
  {#if filters.days.length > 0}
    <FilterChip label={formatDaysChip(filters.days)} onRemove={() => (filters.days = [])} />
  {/if}
  {#if filters.timeStart !== null || filters.timeEnd !== null}
    <FilterChip
      label={formatTimeChip(filters.timeStart, filters.timeEnd)}
      onRemove={() => {
        filters.timeStart = null;
        filters.timeEnd = null;
      }}
    />
  {/if}
  {#each formatChipGroups as group (group.type)}
    <FilterChip label={group.label} onRemove={() => removeFormatGroup(group)} />
  {/each}
  {#if filters.campus.length > 0}
    <FilterChip
      label={formatMultiChip(filters.campus, getCampusFilterLabel)}
      onRemove={() => (filters.campus = [])}
    />
  {/if}
  {#if filters.partOfTerm.length > 0}
    <FilterChip
      label={formatMultiChip(filters.partOfTerm, getPartOfTermFilterLabel)}
      onRemove={() => (filters.partOfTerm = [])}
    />
  {/if}
  {#if filters.attributes.length > 0}
    <FilterChip
      label={formatMultiChip(filters.attributes, getAttributeFilterLabel)}
      onRemove={() => (filters.attributes = [])}
    />
  {/if}
  {#if filters.creditHourMin !== null || filters.creditHourMax !== null}
    <FilterChip
      label={filters.creditHourMin !== null && filters.creditHourMax !== null
        ? `${filters.creditHourMin}–${filters.creditHourMax} credits`
        : filters.creditHourMin !== null
          ? `≥ ${filters.creditHourMin} credits`
          : `≤ ${filters.creditHourMax} credits`}
      onRemove={() => {
        filters.creditHourMin = null;
        filters.creditHourMax = null;
      }}
    />
  {/if}
  {#if filters.instructor !== ""}
    <FilterChip
      label="Instructor: {filters.instructor}"
      onRemove={() => (filters.instructor = "")}
    />
  {/if}
  {#if filters.courseNumberLow !== null || filters.courseNumberHigh !== null}
    <FilterChip
      label={filters.courseNumberLow !== null && filters.courseNumberHigh !== null
        ? `Course ${filters.courseNumberLow}–${filters.courseNumberHigh}`
        : filters.courseNumberLow !== null
          ? `Course ≥ ${filters.courseNumberLow}`
          : `Course ≤ ${filters.courseNumberHigh}`}
      onRemove={() => {
        filters.courseNumberLow = null;
        filters.courseNumberHigh = null;
      }}
    />
  {/if}
  {#if filters.activeCount >= 2}
    <button
      type="button"
      class="text-xs text-muted-foreground hover:text-foreground transition-colors cursor-pointer select-none ml-1 shrink-0"
      onclick={() => filters.clear()}
    >
      Clear all
    </button>
  {/if}
  <!-- Trailing spacer so last chip scrolls past the fade mask -->
  <div class="shrink-0 w-6 md:hidden" aria-hidden="true"></div>
</div>
