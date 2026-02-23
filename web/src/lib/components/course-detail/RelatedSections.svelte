<script lang="ts">
import { client } from "$lib/api";
import type { CourseResponse } from "$lib/bindings";
import {
  abbreviateInstructor,
  formatMeetingTimeSummary,
  getPrimaryInstructor,
  seatsColor,
  seatsDotColor,
} from "$lib/course";
import { getInstructionalMethodLabel } from "$lib/labels";
import { formatNumber } from "$lib/utils";
import { Loader2 } from "@lucide/svelte";
import { getCourseDetailContext } from "./context";

let { course, sectionCount = $bindable(0) }: { course: CourseResponse; sectionCount?: number } =
  $props();

const ctx = getCourseDetailContext();

type LoadState =
  | { mode: "loading" }
  | { mode: "loaded"; sections: CourseResponse[] }
  | { mode: "error"; message: string };

let state = $state<LoadState>({ mode: "loading" });

$effect(() => {
  const { termCode, subject, courseNumber } = course;
  state = { mode: "loading" };

  void client.getRelatedSections(termCode, subject, courseNumber).then((result) => {
    if (result.isErr) {
      state = { mode: "error", message: result.error.message };
    } else {
      const sections = result.value;
      state = { mode: "loaded", sections };
      sectionCount = sections.filter((s) => s.crn !== course.crn).length;
    }
  });
});

// How far the scroll container pulls up behind the header (header height + gap)
const OVERLAP = 24;
// How much of the mask is fully transparent (should cover the header text)
const HIDDEN = 16;
// Width of the transparent-to-opaque fade band
const FADE = 0;
// Extra spacing between the opaque boundary and the first card
const INSET = 4;

const pad = HIDDEN + FADE + INSET;
const fadeMask = `linear-gradient(to bottom, transparent 16px, black ${16}px, black calc(100% - ${24}px), transparent`;
let maskStyle = $derived(sectionCount >= 2 ? fadeMask : "none");

function handleNavigate(crn: string) {
  ctx?.navigateToSection?.(crn);
}
</script>

<div class="flex flex-col gap-2 min-h-0 md:flex-1">
    <h4
        class="text-xs font-medium text-muted-foreground uppercase tracking-wide shrink-0 relative z-10"
    >
        Other Sections
    </h4>

    {#if state.mode === "loading"}
        <div class="flex items-center justify-center py-4">
            <Loader2 class="size-4 text-muted-foreground animate-spin" />
        </div>
    {:else if state.mode === "error"}
        <p class="text-xs text-muted-foreground italic">{state.message}</p>
    {:else if state.mode === "loaded" && state.sections.length <= 1}
        <p class="text-xs text-muted-foreground italic">
            No other sections available.
        </p>
    {:else if state.mode === "loaded"}
        <div
            class="sections-scroll flex flex-col gap-1.5 grow md:overflow-y-auto scrollbar-none"
            style:mask-image={maskStyle}
            style:-webkit-mask-image={maskStyle}
            style:--scroll-overlap="{OVERLAP}px"
            style:--scroll-pad="{pad}px"
        >
            {#each state.sections.filter((s) => s.crn !== course.crn) as section (section.crn)}
                {@const openSeats =
                    section.enrollment.max - section.enrollment.current}
                {@const primary = getPrimaryInstructor(
                    section.instructors,
                    section.primaryInstructorId,
                )}
                <button
                    class="w-full text-left border rounded-md px-2.5 py-1.5 transition-colors cursor-pointer
            border-border bg-card hover:bg-muted/50 shrink-0"
                    onclick={() => handleNavigate(section.crn)}
                >
                    <div class="flex items-center justify-between gap-2">
                        <div class="flex items-center gap-2 min-w-0">
                            <span
                                class="text-xs font-mono text-muted-foreground"
                                >{section.crn}</span
                            >
                            {#if section.instructionalMethod}
                                <span
                                    class="text-xs text-muted-foreground truncate"
                                >
                                    {getInstructionalMethodLabel(section.instructionalMethod, "filter")}
                                </span>
                            {/if}
                        </div>
                        <span
                            class="inline-flex items-center gap-1 shrink-0 text-xs select-none"
                        >
                            <span
                                class="size-1.5 rounded-full {seatsDotColor(
                                    openSeats,
                                )} shrink-0"
                            ></span>
                            <span
                                class="{seatsColor(
                                    openSeats,
                                )} font-medium tabular-nums"
                            >
                                {#if openSeats === 0}Full{:else}{openSeats}/{formatNumber(
                                        section.enrollment.max,
                                    )}{/if}
                            </span>
                        </span>
                    </div>

                    <div class="flex items-center justify-between gap-2 mt-0.5">
                        <span class="text-xs text-muted-foreground truncate">
                            {abbreviateInstructor(
                                primary?.displayName ?? "Staff",
                            )}
                            {#if primary?.rmp?.avgRating != null}
                                <span class="font-medium text-foreground/70"
                                    >{primary.rmp.avgRating.toFixed(1)}</span
                                >
                            {/if}
                        </span>
                        <span class="text-xs text-muted-foreground shrink-0">
                            {formatMeetingTimeSummary(section)}
                        </span>
                    </div>

                    <!-- eslint-disable-next-line @typescript-eslint/prefer-nullish-coalescing -->
                    {#if section.primaryLocation || (section.enrollment.waitCapacity > 0 && section.enrollment.waitCount > 0)}
                        <div
                            class="flex items-center justify-between gap-2 mt-0.5"
                        >
                            {#if section.primaryLocation}
                                <span
                                    class="text-xs text-muted-foreground/70 truncate"
                                >
                                    {section.primaryLocation}
                                </span>
                            {:else}
                                <span></span>
                            {/if}
                            {#if section.enrollment.waitCapacity > 0 && section.enrollment.waitCount > 0}
                                <span
                                    class="text-xs text-muted-foreground/70 shrink-0"
                                >
                                    Wait: {formatNumber(
                                        section.enrollment.waitCount,
                                    )}/{formatNumber(
                                        section.enrollment.waitCapacity,
                                    )}
                                </span>
                            {/if}
                        </div>
                    {/if}
                </button>
            {/each}
        </div>
    {/if}
</div>

<style>
    @media (min-width: 768px) {
        .sections-scroll {
            margin-top: calc(-1 * var(--scroll-overlap));
            padding-top: var(--scroll-pad);
            padding-bottom: var(--scroll-pad);
        }
    }
</style>
