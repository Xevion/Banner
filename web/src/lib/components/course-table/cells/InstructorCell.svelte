<script lang="ts">
import type { CourseResponse } from "$lib/bindings";
import ScorePopover from "$lib/components/score/ScorePopover.svelte";
import { abbreviateInstructor, getPrimaryInstructor, ratingColor } from "$lib/course";
import { unassignedInstructorDetail } from "$lib/scheduleState";
import { themeStore } from "$lib/stores/theme.svelte";

let { course }: { course: CourseResponse } = $props();

let primary = $derived(getPrimaryInstructor(course.instructors, course.primaryInstructorId));
let display = $derived(primary ? abbreviateInstructor(primary.displayName) : "");
let commaIdx = $derived(display.indexOf(", "));
let profileUrl = $derived(primary?.slug ? `/instructors/${primary.slug}` : null);

let rating = $derived(primary?.rating ?? null);
let hue = $derived(rating ? ratingColor(rating.score, themeStore.isDark) : null);
</script>

{#snippet name()}
  {#if commaIdx !== -1}
    <span class="truncate"
      >{display.slice(0, commaIdx)},<span class="text-muted-foreground"
        >{display.slice(commaIdx + 1)}</span
      ></span
    >
  {:else}
    <span class="truncate">{display}</span>
  {/if}
{/snippet}

<td class="max-w-0 px-2 align-middle">
  <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
    <!-- "Unassigned" says what Banner's own "Staff" filler does not, and the
         tooltip says why: an unclaimed independent-study shell is not the same
         absence as a scheduled section still missing an assignment. -->
    {#if !primary}
      <span
        class="truncate text-[12.5px] text-muted-foreground/50 italic select-none"
        data-tooltip={unassignedInstructorDetail(course)}
        data-tooltip-side="bottom"
        data-tooltip-delay="200">Unassigned</span
      >
    {:else if profileUrl}
      <a
        href={profileUrl}
        data-tooltip={primary.displayName}
        data-tooltip-side="bottom"
        data-tooltip-delay="200"
        class="min-w-0 text-[12.5px] hover:underline"
      >
        {@render name()}
      </a>
    {:else}
      <span
        class="min-w-0 text-[12.5px]"
        data-tooltip={primary.displayName}
        data-tooltip-side="bottom"
        data-tooltip-delay="200"
      >
        {@render name()}
      </span>
    {/if}

    <!-- Fixed-width slot so the numbers align down the column; an unrated
         section leaves it empty rather than showing a placeholder dash. -->
    <span class="w-8 text-right">
      {#if rating && hue}
        <ScorePopover rating={rating} rmp={primary?.rmp} bluebook={primary?.bluebook}>
          <span
            class="inline-block w-8 rounded-[4px] py-[1.5px] text-center font-mono text-[10.5px] font-semibold tabular-nums"
            style:background-color="color-mix(in oklab, {hue} 15%, transparent)"
            style:color="color-mix(in oklab, {hue} 72%, var(--foreground))"
          >
            {rating.score.toFixed(1)}
          </span>
        </ScorePopover>
      {/if}
    </span>
  </div>
</td>
