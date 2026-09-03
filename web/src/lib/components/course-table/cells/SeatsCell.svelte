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

<td class="px-2 align-middle whitespace-nowrap">
  <span
    class="grid grid-cols-[1.5rem_2.25rem_minmax(0,1fr)] items-baseline gap-x-[5px] select-none"
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
  </span>
</td>
