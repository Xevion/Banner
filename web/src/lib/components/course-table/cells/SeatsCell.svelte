<script lang="ts">
import type { CourseResponse } from "$lib/bindings";
import { seatsColor, seatsDotColor } from "$lib/course";
import { formatNumber } from "$lib/utils";

let { course }: { course: CourseResponse } = $props();

let open = $derived(course.enrollment.max - course.enrollment.current);
let seatsTip = $derived(
  open < 0
    ? `Overenrolled by ${Math.abs(open)} &mdash; ${formatNumber(course.enrollment.current)}/${formatNumber(course.enrollment.max)} enrolled${course.enrollment.waitCount > 0 ? `, ${formatNumber(course.enrollment.waitCount)} waitlisted` : ""}`
    : `${formatNumber(open)} of ${formatNumber(course.enrollment.max)} seats open, ${formatNumber(course.enrollment.current)} enrolled${course.enrollment.waitCount > 0 ? `, ${formatNumber(course.enrollment.waitCount)} waitlisted` : ""}`
);
</script>

<td class="py-2 px-2 text-right whitespace-nowrap">
  <span
    class="inline-flex items-center gap-1.5 select-none"
    data-tooltip={seatsTip}
    data-tooltip-side="left"
    data-tooltip-delay="200"
  >
    <span class="size-1.5 rounded-full {seatsDotColor(open)} shrink-0"></span>
    <span class="{seatsColor(open)} font-medium tabular-nums"
      >{#if open < 0}Overenrolled{:else if open === 0}Full{:else}{open} open{/if}</span
    >
    <span class="text-muted-foreground/60 tabular-nums"
      >{formatNumber(course.enrollment.current)}/{formatNumber(course.enrollment.max)}{#if course.enrollment.waitCount > 0}
        &middot; WL {formatNumber(course.enrollment.waitCount)}/{formatNumber(course.enrollment.waitCapacity)}{/if}</span
    >
  </span>
</td>
