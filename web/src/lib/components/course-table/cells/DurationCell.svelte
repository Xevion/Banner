<script lang="ts">
import type { CourseResponse, DbMeetingTime } from "$lib/bindings";
import { formatDuration, meetingDurationMinutes, parseTimeMinutes } from "$lib/schedule";

let { course }: { course: CourseResponse } = $props();

/** Matches TimeCell's choice of meeting, so the duration describes the time beside it. */
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

let minutes = $derived.by(() => {
  const lead = earliestTimedMeeting(course.meetingTimes);
  return lead ? meetingDurationMinutes(lead) : null;
});
</script>

<td class="truncate px-2 text-right align-middle font-mono text-[11px] tabular-nums">
  {#if minutes !== null}
    <span class="text-muted-foreground select-none">{formatDuration(minutes)}</span>
  {:else}
    <span class="text-muted-foreground/40 select-none">N/A</span>
  {/if}
</td>
