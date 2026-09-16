<script lang="ts">
import type { CourseResponse } from "$lib/bindings";
import {
  abbreviateInstructor,
  formatMeetingTimeSummary,
  getPrimaryInstructor,
  seatsColor,
  seatsDotColor,
} from "$lib/course";
import { formatNumber } from "$lib/utils";
import type { TransitionConfig } from "svelte/transition";
import { slide } from "svelte/transition";
import CourseDetail from "./CourseDetail.svelte";

/** Slide transition that gracefully skips when the element is hidden (display: none). */
function safeSlide(node: Element, params: Parameters<typeof slide>[1] = {}): TransitionConfig {
  if ((node as HTMLElement).offsetParent === null) {
    return { duration: 0 };
  }
  return slide(node, params);
}

let {
  course,
  expanded,
  onToggle,
}: {
  course: CourseResponse;
  expanded: boolean;
  onToggle: () => void;
} = $props();

let primary = $derived(getPrimaryInstructor(course.instructors, course.primaryInstructorId));
let instructorName = $derived(abbreviateInstructor(primary?.displayName ?? "Staff"));
let profileUrl = $derived(primary?.slug ? `/instructors/${primary.slug}` : null);
</script>

<div
  class="rounded-lg border border-border bg-card overflow-hidden transition-colors
    {expanded ? 'border-border/80' : 'hover:bg-muted/30'}"
>
  <!-- The toggle is a full-bleed overlay rather than a wrapper so the
       instructor link can live inside the card without nesting in a button. -->
  <div class="relative">
    <button
      class="absolute inset-0 cursor-pointer"
      aria-expanded={expanded}
      aria-label="{expanded ? 'Hide' : 'Show'} details for {course.subject} {course.courseNumber}"
      onclick={onToggle}
    ></button>
    <div class="relative p-3 pointer-events-none">
      <!-- Line 1: Course code + title + seats -->
      <div class="flex items-baseline justify-between gap-2">
        {#snippet seatsDisplay()}
          {@const openSeats = course.enrollment.max - course.enrollment.current}
          <span class="inline-flex items-center gap-1 shrink-0 text-xs select-none">
            <span class="size-1.5 rounded-full {seatsDotColor(openSeats)} shrink-0"></span>
            <span class="{seatsColor(openSeats)} font-medium tabular-nums">
              {#if openSeats < 0}Over{:else if openSeats === 0}Full{:else}{openSeats}/{formatNumber(course.enrollment.max)}{/if}
            </span>
          </span>
        {/snippet}
        <div class="flex items-baseline gap-1.5 min-w-0">
          <span class="font-mono font-semibold text-sm tracking-tight shrink-0">
            {course.subject} {course.courseNumber}
          </span>
          <span class="text-sm text-muted-foreground truncate">{course.title}</span>
        </div>
        {@render seatsDisplay()}
      </div>

      <!-- Line 2: Instructor + time -->
      <div class="flex items-center justify-between gap-2 mt-1">
        {#if profileUrl}
          <a
            href={profileUrl}
            class="text-xs text-muted-foreground truncate pointer-events-auto hover:text-foreground hover:underline"
          >
            {instructorName}
          </a>
        {:else}
          <span class="text-xs text-muted-foreground truncate">{instructorName}</span>
        {/if}
        <span class="text-xs text-muted-foreground shrink-0">
          {formatMeetingTimeSummary(course)}
        </span>
      </div>
    </div>
  </div>

  {#if expanded}
    <div transition:safeSlide={{ duration: 200 }}>
      <CourseDetail {course} />
    </div>
  {/if}
</div>
