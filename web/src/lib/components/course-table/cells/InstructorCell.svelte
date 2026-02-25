<script lang="ts">
import type { CourseResponse } from "$lib/bindings";
import ScorePopover from "$lib/components/score/ScorePopover.svelte";
import { abbreviateInstructor, getPrimaryInstructor } from "$lib/course";

let { course }: { course: CourseResponse } = $props();

let primary = $derived(getPrimaryInstructor(course.instructors, course.primaryInstructorId));
let display = $derived(primary ? abbreviateInstructor(primary.displayName) : "Staff");
let commaIdx = $derived(display.indexOf(", "));
let profileUrl = $derived(primary?.slug ? `/instructors/${primary.slug}` : null);
</script>

<td class="py-2 px-2 whitespace-nowrap">
  {#if display === "Staff"}
    <span class="text-xs text-muted-foreground/60 uppercase select-none">Staff</span>
  {:else}
    {#if profileUrl}
      <a
        href={profileUrl}
        data-tooltip={primary?.displayName ?? "Staff"}
        data-tooltip-side="bottom"
        data-tooltip-delay="200"
        class="hover:underline"
      >
        {#if commaIdx !== -1}
          <span
            >{display.slice(0, commaIdx)},
            <span class="text-muted-foreground"
              >{display.slice(commaIdx + 1)}</span
            ></span
          >
        {:else}
          <span>{display}</span>
        {/if}
      </a>
    {:else}
      <span
        data-tooltip={primary?.displayName ?? "Staff"}
        data-tooltip-side="bottom"
        data-tooltip-delay="200"
      >
        {#if commaIdx !== -1}
          <span
            >{display.slice(0, commaIdx)},
            <span class="text-muted-foreground"
              >{display.slice(commaIdx + 1)}</span
            ></span
          >
        {:else}
          <span>{display}</span>
        {/if}
      </span>
    {/if}
  {/if}
  {#if primary?.composite}
    <span class="ml-1">
      <ScorePopover
        composite={primary.composite}
        rmp={primary.rmp}
        bluebook={primary.bluebook}
        size="xs"
      />
    </span>
  {/if}
</td>
