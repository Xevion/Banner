<script lang="ts">
import type { CourseResponse } from "$lib/bindings";
import { getTableContext } from "../context";

let { course }: { course: CourseResponse } = $props();

const { subjectMap, maxSubjectLength } = getTableContext();

let subjectDesc = $derived(subjectMap[course.subject]);
let paddedSubject = $derived(course.subject.padStart(maxSubjectLength, " "));
let coursePath = $derived(`/courses/${course.termSlug}/${course.subject}/${course.courseNumber}`);
</script>

<td class="px-2 align-middle whitespace-nowrap">
  <a
    href={coursePath}
    data-tooltip={subjectDesc
      ? `${subjectDesc} ${course.courseNumber}`
      : `${course.subject} ${course.courseNumber}`}
    data-tooltip-side="bottom"
    data-tooltip-delay="200"
    class="font-mono text-[12.5px] font-semibold tracking-[-0.02em] tabular-nums whitespace-pre transition-colors hover:underline"
    >{paddedSubject} {course.courseNumber}{#if course.sequenceNumber}<span
        class="font-normal text-muted-foreground">-{course.sequenceNumber}</span
      >{/if}</a
  >
</td>
