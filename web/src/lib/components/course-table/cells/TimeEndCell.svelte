<script lang="ts">
import type { CourseResponse, DbMeetingTime } from "$lib/bindings";
import { formatMeetingTimesTooltip, formatTime } from "$lib/course";
import { parseTimeMinutes } from "$lib/schedule";
import { scheduleCopy, scheduleState } from "$lib/scheduleState";
import { getTableContext } from "../context";

let { course }: { course: CourseResponse } = $props();

const { isColumnVisible } = getTableContext();

/** Matches TimeCell's choice of meeting, so both halves describe the same one. */
function earliestTimedMeeting(meetingTimes: DbMeetingTime[]): DbMeetingTime | null {
  let earliest: DbMeetingTime | null = null;
  let earliestStart = Number.POSITIVE_INFINITY;
  for (const mt of meetingTimes) {
    const start = parseTimeMinutes(mt.timeRange?.start ?? null);
    if (start === null || start >= earliestStart) continue;
    earliest = mt;
    earliestStart = start;
  }
  return earliest;
}

let copy = $derived(scheduleCopy(scheduleState(course)));
let lead = $derived(earliestTimedMeeting(course.meetingTimes));
// Beside a visible start the two columns read as one, so this cell is the back
// half of a range and never states anything on its own: an untimed row is
// already fully described by the phrase in the start cell.
let paired = $derived(isColumnVisible("time"));
</script>

<td
  class="truncate py-0 pr-2 align-middle {paired && !copy ? 'pl-0 text-left' : 'pl-2 text-right'}"
  data-tooltip={copy?.detail ?? formatMeetingTimesTooltip(course.meetingTimes)}
>
  {#if copy}
    {#if !paired}
      <span class="font-mono text-[11px] text-muted-foreground/40 tabular-nums select-none"
        >N/A</span
      >
    {/if}
  {:else}
    <span class="font-mono text-[11.5px] font-medium tabular-nums select-none"
      >{#if paired}<span class="px-[3px] text-muted-foreground">&ndash;</span>{/if}{formatTime(
        lead?.timeRange?.end ?? null
      )}</span
    >
  {/if}
</td>
