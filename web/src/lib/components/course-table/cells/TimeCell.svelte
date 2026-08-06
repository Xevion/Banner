<script lang="ts">
import type { CourseResponse, DbMeetingTime } from "$lib/bindings";
import { formatISOTime, formatMeetingTimesTooltip } from "$lib/course";
import {
  SCHEDULE_DAY_LABELS,
  formatDuration,
  meetingDayFlags,
  meetingDurationMinutes,
  meetingTrackSpans,
  parseTimeMinutes,
} from "$lib/schedule";

let { course }: { course: CourseResponse } = $props();

const ACTIVE_DAY = "var(--status-blue)";
const INACTIVE_DAY = "color-mix(in oklab, var(--foreground) 13%, var(--card))";

/** The meeting whose start drives the label; the track still shows all of them. */
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

let dayFlags = $derived(meetingDayFlags(course.meetingTimes));
let spans = $derived(meetingTrackSpans(course.meetingTimes));
let lead = $derived(earliestTimedMeeting(course.meetingTimes));
let startLabel = $derived(lead?.timeRange ? formatISOTime(lead.timeRange.start) : null);
let duration = $derived(lead ? meetingDurationMinutes(lead) : null);
</script>

<td
  class="py-2 px-2 whitespace-nowrap align-middle"
  data-tooltip={formatMeetingTimesTooltip(course.meetingTimes)}
>
  <div class="flex w-31 flex-col gap-[5px] select-none">
    <div class="flex gap-0.5">
      {#each dayFlags as active, i (i)}
        <span
          class="flex size-4 items-center justify-center rounded text-[9px] leading-none font-bold tracking-[-0.03em] text-white"
          style:background-color={active ? ACTIVE_DAY : INACTIVE_DAY}
        >
          {active ? SCHEDULE_DAY_LABELS[i] : ""}
        </span>
      {/each}
    </div>

    {#if spans.length > 0}
      <div
        class="day-track relative h-[9px] w-full overflow-hidden rounded-[2px] border border-border bg-muted"
      >
        {#each spans as span, i (i)}
          <div
            class="absolute inset-y-0 overflow-hidden bg-status-blue"
            style:left="{span.left}%"
            style:width="{span.width}%"
          >
            <!-- Inverse-scaled to the track's own box so the gridlines stay on the
                 segment boundaries while being clipped to the active range. -->
            <div
              class="day-track-gridlines pointer-events-none absolute inset-y-0"
              style:left="{(-span.left / span.width) * 100}%"
              style:width="{(100 / span.width) * 100}%"
            ></div>
          </div>
        {/each}
      </div>
    {/if}

    <span class="font-mono text-[11px] text-muted-foreground">
      {#if course.isAsyncOnline}
        Async &middot; online
      {:else if startLabel && duration !== null}
        {startLabel} &middot; {formatDuration(duration)}
      {:else}
        TBA
      {/if}
    </span>
  </div>
</td>

<style>
/* Three-hour segment dividers across the 7am-10pm window. Tailwind can't express
   a repeating gradient without an unreadable arbitrary value. */
.day-track {
  background-image: repeating-linear-gradient(
    to right,
    transparent 0,
    transparent calc(20% - 1px),
    color-mix(in oklab, var(--foreground) 14%, transparent) calc(20% - 1px),
    color-mix(in oklab, var(--foreground) 14%, transparent) 20%
  );
}

/* Same dividers repainted in white so they stay visible over the active range. */
.day-track-gridlines {
  background-image: repeating-linear-gradient(
    to right,
    transparent 0,
    transparent calc(20% - 1px),
    rgba(255, 255, 255, 0.6) calc(20% - 1px),
    rgba(255, 255, 255, 0.6) 20%
  );
}
</style>
