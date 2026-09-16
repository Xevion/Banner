<script lang="ts">
import type { CourseResponse } from "$lib/bindings";
import InstructorCard from "$lib/components/InstructorCard.svelte";
import { formatInstructorName } from "$lib/course";

let { course }: { course: CourseResponse } = $props();
</script>

<div>
  <h4 class="text-xs font-medium text-muted-foreground uppercase tracking-wide mb-1.5">
    Instructors
  </h4>
  {#if course.instructors.length > 0}
    <div class="flex flex-col gap-1.5">
      {#each course.instructors as instructor (instructor.instructorId)}
        <InstructorCard
          variant="compact"
          name={formatInstructorName(instructor)}
          slug={instructor.slug}
          rating={instructor.rating}
          rmp={instructor.rmp}
          bluebook={instructor.bluebook}
          badge={instructor.isPrimary && course.instructors.length > 1 ? "Primary" : null}
        />
      {/each}
    </div>
  {:else}
    <span class="italic text-muted-foreground text-sm">Staff</span>
  {/if}
</div>
