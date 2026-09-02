<script lang="ts">
import type { CourseResponse } from "$lib/bindings";
import { seatsColor } from "$lib/course";
import { formatNumber } from "$lib/utils";

let { course }: { course: CourseResponse } = $props();

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

<td class="py-2 px-2 text-right whitespace-nowrap">
  <span
    class="inline-flex flex-col items-end select-none"
    data-tooltip={seatsTip}
    data-tooltip-side="left"
    data-tooltip-delay="200"
  >
    <span class="text-xl leading-[1.05] font-bold tracking-[-0.02em] tabular-nums {countColor}">
      {formatNumber(open)}
    </span>
    <span class="text-[10px] text-muted-foreground tabular-nums"
      >of {formatNumber(course.enrollment.max)}</span
    >
    {#if waitlisted > 0}
      <span class="text-[10px] font-medium text-seat-over tabular-nums"
        >{formatNumber(waitlisted)} waitlisted</span
      >
    {/if}
  </span>
</td>
