<script lang="ts">
import type { CourseResponse } from "$lib/bindings";
import { formatMeetingDays, formatMeetingTimesTooltip, formatTimeRange } from "$lib/course";

let { course }: { course: CourseResponse } = $props();

function timeIsTBA(c: CourseResponse): boolean {
  if (c.meetingTimes.length === 0) return true;
  const mt = c.meetingTimes[0];
  return mt.days.length === 0 && mt.timeRange === null;
}
</script>

<td
  class="py-2 px-2 whitespace-nowrap"
  data-tooltip={formatMeetingTimesTooltip(course.meetingTimes)}
>
  {#if course.isAsyncOnline}
    <span class="text-xs text-muted-foreground/60 select-none">Async</span>
  {:else if timeIsTBA(course)}
    <span class="text-xs text-muted-foreground/60 select-none">TBA</span>
  {:else}
    {@const mt = course.meetingTimes[0]}
    <span>
      {#if mt.days.length > 0}
        <span class="font-mono font-medium">{formatMeetingDays(mt)}</span> 
      {/if}
      {#if mt.timeRange !== null}
        <span class="text-muted-foreground"
          >{formatTimeRange(mt.timeRange.start, mt.timeRange.end)}</span
        >
      {:else}
        <span class="text-xs text-muted-foreground/60 select-none">TBA</span>
      {/if}
      {#if course.meetingTimes.length > 1}
        <span class="ml-1 text-xs text-muted-foreground/70 font-medium select-none"
          >+{course.meetingTimes.length - 1}</span
        >
      {/if}
    </span>
  {/if}
</td>
