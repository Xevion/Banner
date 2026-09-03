<script lang="ts">
import type { CourseResponse, DbMeetingTime } from "$lib/bindings";
import { formatMeetingTimesTooltip, formatTimeParts } from "$lib/course";
import { parseTimeMinutes } from "$lib/schedule";
import { scheduleCopy, scheduleState } from "$lib/scheduleState";
import { getTableContext } from "../context";

let { course }: { course: CourseResponse } = $props();

const { isColumnVisible } = getTableContext();

/** The meeting whose start drives the label, when a section meets more than once. */
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
let parts = $derived(formatTimeParts(lead?.timeRange?.start ?? null));
// Only two real times form a range worth closing the gap for. An untimed row
// keeps its padding, or the phrase collides with whatever the end cell shows.
let joined = $derived(!copy && isColumnVisible("time_end"));
// An untimed row has one thing to say across both halves, so this cell covers
// the pair and the row skips the end cell. See `timeSpansPair`.
let spans = $derived(!!copy && isColumnVisible("time_end"));
</script>

<td
  colspan={spans ? 2 : undefined}
  class="truncate py-0 pl-2 align-middle {joined ? 'pr-0' : 'pr-2'} {spans
    ? 'text-center'
    : 'text-right'}"
  data-tooltip={copy?.detail ?? formatMeetingTimesTooltip(course.meetingTimes)}
>
  <!-- Centred only when it spans the pair, where there is room to sit under the
       whole region. In one column it nearly fills the box, so centring floats it
       off the edge the times end on; there it shares that edge instead. -->
  {#if copy}
    <span class="text-[11.5px] text-muted-foreground/80 italic select-none">{copy.phrase}</span>
  {:else if parts}
    <!-- Fixed ch tracks: hour right-aligned against a two-digit hour, then the
         colon and meridiem each pinned to their own column down the page. -->
    <span
      class="inline-grid grid-cols-[2ch_3ch_3ch] text-left font-mono text-[11.5px] font-medium tabular-nums select-none"
    >
      <span class="text-right">{parts.hour}</span>
      <span>:{parts.minute}</span>
      <span class="pl-[1ch]">{parts.meridiem}</span>
    </span>
  {/if}
</td>
