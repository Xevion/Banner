<script lang="ts">
import type { CourseResponse } from "$lib/bindings";

let { course }: { course: CourseResponse } = $props();

let coursePath = $derived(`/courses/${course.termSlug}/${course.subject}/${course.courseNumber}`);
</script>

<td class="max-w-0 px-2 align-middle">
  <a
    href={coursePath}
    class="block truncate text-[12.5px] transition-colors hover:underline"
    data-tooltip={course.title}
    data-tooltip-side="bottom"
    data-tooltip-delay="200">{course.title}</a
  >
</td>
