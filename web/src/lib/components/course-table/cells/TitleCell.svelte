<script lang="ts">
import type { CourseResponse } from "$lib/bindings";

let { course }: { course: CourseResponse } = $props();
</script>

<td class="py-2 px-2 font-medium max-w-50 truncate">
  <span
    class="block truncate"
    data-tooltip={course.title}
    data-tooltip-side="bottom"
    data-tooltip-delay="200"
  >{course.title}</span>
</td>
