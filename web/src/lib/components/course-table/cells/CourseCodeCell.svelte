<script lang="ts">
import type { CourseResponse } from "$lib/bindings";
import { getTableContext } from "../context";

let { course }: { course: CourseResponse } = $props();

const { subjectMap, maxSubjectLength } = getTableContext();

let subjectDesc = $derived(subjectMap[course.subject]);
let paddedSubject = $derived(course.subject.padStart(maxSubjectLength, " "));
</script>

<td class="py-2 px-2 whitespace-nowrap">
  <a
    href="/courses/{course.termSlug}/{course.subject}/{course.courseNumber}"
    data-tooltip={subjectDesc
      ? `${subjectDesc} ${course.courseNumber}`
      : `${course.subject} ${course.courseNumber}`}
    data-tooltip-side="bottom"
    data-tooltip-delay="200"
    class="hover:underline hover:text-foreground transition-colors"
  >
    <span class="font-semibold font-mono tracking-tight whitespace-pre">{paddedSubject} {course.courseNumber}</span>{#if course.sequenceNumber}<span class="text-muted-foreground font-mono tracking-tight">-{course.sequenceNumber}</span>{/if}
  </a>
</td>
