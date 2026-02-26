<script lang="ts">
import type { CourseResponse } from "$lib/bindings";

let { course }: { course: CourseResponse } = $props();
</script>

<td class="py-2 px-2 font-medium max-w-50 truncate">
  <a
    href="/courses/{course.termSlug}/{course.subject}/{course.courseNumber}"
    class="block truncate hover:underline hover:text-foreground transition-colors"
    data-tooltip={course.title}
    data-tooltip-side="bottom"
    data-tooltip-delay="200"
  >{course.title}</a>
</td>
