<script lang="ts">
import type { CourseResponse } from "$lib/bindings";
import { seatsColor } from "$lib/course";
import { courseTrends } from "$lib/stores/course-trends.svelte";
import { formatNumber } from "$lib/utils";
import SeatsTrend from "./SeatsTrend.svelte";

let { course }: { course: CourseResponse } = $props();

// Absent for the ~97% of sections the scraper has never seen change; the cell just
// renders without it rather than showing an empty slot.
let trend = $derived(courseTrends.get(course.termSlug, course.crn));

let open = $derived(course.enrollment.max - course.enrollment.current);
let waitlisted = $derived(course.enrollment.waitCount);

// A section with a waitlist has no truly free seats, whatever the open count says --
// but overenrollment outranks that, since only it says how far past full the section is.
let countColor = $derived(open >= 0 && waitlisted > 0 ? "text-seat-full" : seatsColor(open));

let seatsTip = $derived(
  open < 0
    ? `Overenrolled by ${Math.abs(open)} \u2014 ${formatNumber(course.enrollment.current)}/${formatNumber(course.enrollment.max)} enrolled${waitlisted > 0 ? `, ${formatNumber(waitlisted)} waitlisted` : ""}`
    : `${formatNumber(open)} of ${formatNumber(course.enrollment.max)} seats open, ${formatNumber(course.enrollment.current)} enrolled${waitlisted > 0 ? `, ${formatNumber(waitlisted)} waitlisted` : ""}`
);
</script>

<td class="px-2 align-middle whitespace-nowrap">
  <span
    class="grid grid-cols-[1.5rem_2.25rem_2.5rem_2rem] items-baseline gap-x-[5px] select-none"
    data-tooltip={seatsTip}
    data-tooltip-side="left"
    data-tooltip-delay="200"
  >
    <span
      class="text-right text-sm leading-none font-bold tracking-[-0.01em] tabular-nums {countColor}"
      >{formatNumber(open)}</span
    >
    <span class="text-[11px] text-muted-foreground tabular-nums"
      >of {formatNumber(course.enrollment.max)}</span
    >
    <span class="font-mono text-[10px] text-seat-over tabular-nums">
      {#if waitlisted > 0}wl {formatNumber(waitlisted)}{/if}
    </span>
    <span class="self-center justify-self-end">
      {#if trend && trend.length > 1}
        <SeatsTrend samples={trend} />
      {/if}
    </span>
  </span>
</td>
