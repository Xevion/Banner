<script lang="ts">
import type { CourseResponse } from "$lib/bindings";
import { formatMeetingTimesTooltip } from "$lib/course";
import { SCHEDULE_DAY_LABELS, meetingDayFlags } from "$lib/schedule";
import { scheduleCopy, scheduleState } from "$lib/scheduleState";
import { Ban, Play, UserRound } from "@lucide/svelte";

let { course }: { course: CourseResponse } = $props();

const ACTIVE_DAY = "var(--status-blue)";
const INACTIVE_DAY = "color-mix(in oklab, var(--foreground) 13%, var(--card))";

let dayFlags = $derived(meetingDayFlags(course.meetingTimes));
let state = $derived(scheduleState(course));
let copy = $derived(scheduleCopy(state));
// One outlined shape each, all stroked at the same weight. At this size interior
// detail smudges, and a filled glyph beside stroked ones reads as a different set.
let Icon = $derived(state === "async" ? Play : state === "arranged" ? UserRound : Ban);
</script>

<td
  class="px-2 align-middle whitespace-nowrap"
  data-tooltip={copy?.detail ?? formatMeetingTimesTooltip(course.meetingTimes)}
>
  {#if copy}
    <!-- Same 110px footprint as the day strip, so the badge sits on the grid the
         chips establish rather than stretching to the column. -->
    <span
      class="flex h-4 w-[110px] items-center justify-center gap-1 rounded-[3px] bg-muted text-[8.5px] font-semibold tracking-[0.1em] text-muted-foreground select-none"
    >
      <Icon class="size-[11px] shrink-0 opacity-80" strokeWidth={2.5} />
      {copy.badge}
    </span>
  {:else}
    <div class="flex gap-[2px] select-none">
      {#each dayFlags as active, i (i)}
        <span
          class="flex h-4 w-[14px] shrink-0 items-center justify-center rounded-[3px] text-[9px] leading-none font-bold tracking-[-0.03em] text-white"
          style:background-color={active ? ACTIVE_DAY : INACTIVE_DAY}
        >
          {active ? SCHEDULE_DAY_LABELS[i] : ""}
        </span>
      {/each}
    </div>
  {/if}
</td>
