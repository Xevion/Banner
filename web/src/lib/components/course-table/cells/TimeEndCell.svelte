<script lang="ts">
import type { CourseResponse, DbMeetingTime } from "$lib/bindings";
import { formatMeetingTimesTooltip, formatTimeParts } from "$lib/course";
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
let parts = $derived(formatTimeParts(lead?.timeRange?.end ?? null));
// Beside a visible start the two columns read as one, so this cell is the back
// half of a range and never states anything on its own: an untimed row is
// already fully described by the phrase in the start cell.
let paired = $derived(isColumnVisible("time"));
</script>

<td
  class="truncate py-0 pr-2 align-middle {paired && !copy ? 'pl-0 text-left' : 'pl-2 text-right'}"
  data-tooltip={copy?.detail ?? formatMeetingTimesTooltip(course.meetingTimes)}
>
  <!-- Only reached when this column stands alone: paired, the start cell spans
       both and the row skips this one. -->
  {#if copy}
    <span class="text-[11.5px] text-muted-foreground/80 italic select-none">{copy.phrase}</span>
  {:else if parts}
    <!-- Same tracks as the start, with the separator given one of its own so it
         lands in the same place on every row. -->
    <span
      class="inline-grid text-left font-mono text-[11.5px] font-medium tabular-nums select-none {paired
        ? 'grid-cols-[3ch_2ch_3ch_3ch]'
        : 'grid-cols-[2ch_3ch_3ch]'}"
    >
      {#if paired}<span class="text-center text-muted-foreground">&ndash;</span>{/if}
      <span class="text-right">{parts.hour}</span>
      <span>:{parts.minute}</span>
      <span class="pl-[1ch]">{parts.meridiem}</span>
    </span>
  {/if}
</td>
