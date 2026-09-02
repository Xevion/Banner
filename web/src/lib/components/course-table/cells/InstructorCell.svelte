<script lang="ts">
import type { CourseResponse } from "$lib/bindings";
import ScorePopover from "$lib/components/score/ScorePopover.svelte";
import {
  abbreviateInstructor,
  concernAccentClass,
  formatLocationTooltip,
  getPrimaryInstructor,
} from "$lib/course";

let { course }: { course: CourseResponse } = $props();

let primary = $derived(getPrimaryInstructor(course.instructors, course.primaryInstructorId));
let display = $derived(primary ? abbreviateInstructor(primary.displayName) : "Staff");
let commaIdx = $derived(display.indexOf(", "));
let profileUrl = $derived(primary?.slug ? `/instructors/${primary.slug}` : null);

let accentClass = $derived(concernAccentClass(course.instructionalMethod, course.campus));
let locTooltip = $derived(formatLocationTooltip(course));
</script>

{#snippet name()}
  {#if commaIdx !== -1}
    <span
      >{display.slice(0, commaIdx)},
      <span class="text-muted-foreground">{display.slice(commaIdx + 1)}</span></span
    >
  {:else}
    <span>{display}</span>
  {/if}
{/snippet}

<td class="py-2 px-2 whitespace-nowrap">
  <div class="flex flex-col gap-px">
    <span class="inline-flex items-center gap-1">
      {#if display === "Staff"}
        <span class="text-xs text-muted-foreground/60 uppercase select-none">Staff</span>
      {:else if profileUrl}
        <a
          href={profileUrl}
          data-tooltip={primary?.displayName ?? "Staff"}
          data-tooltip-side="bottom"
          data-tooltip-delay="200"
          class="text-sm hover:underline"
        >
          {@render name()}
        </a>
      {:else}
        <span
          class="text-sm"
          data-tooltip={primary?.displayName ?? "Staff"}
          data-tooltip-side="bottom"
          data-tooltip-delay="200"
        >
          {@render name()}
        </span>
      {/if}
      {#if primary?.rating}
        <ScorePopover
          rating={primary.rating}
          rmp={primary.rmp}
          bluebook={primary.bluebook}
          size="xs"
        />
      {/if}
    </span>

    {#if course.primaryLocation}
      <span
        class="text-xs text-muted-foreground {accentClass ?? ''}"
        class:pl-2={accentClass !== null}
        data-tooltip={locTooltip}
        data-tooltip-delay="200"
      >
        {course.primaryLocation}
      </span>
    {:else}
      <span class="text-xs text-muted-foreground/50">&mdash;</span>
    {/if}
  </div>
</td>
