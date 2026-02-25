<script lang="ts">
import type { CourseResponse } from "$lib/bindings";
import ScorePopover from "$lib/components/score/ScorePopover.svelte";
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
        <div
          class="flex items-center flex-wrap gap-x-3 gap-y-1 border border-border rounded-md px-3 py-1.5 bg-card"
        >
          <!-- Name + primary badge -->
          <div class="flex items-center gap-2 min-w-0">
            {#if instructor.slug != null}
              <a
                href="/instructors/{instructor.slug}"
                class="font-medium text-sm text-foreground truncate hover:underline"
              >
                {formatInstructorName(instructor)}
              </a>
            {:else}
              <span class="font-medium text-sm text-foreground truncate">
                {formatInstructorName(instructor)}
              </span>
            {/if}
            {#if instructor.isPrimary && course.instructors.length > 1}
              <span
                class="text-[10px] font-medium text-muted-foreground bg-muted rounded px-1.5 py-0.5 shrink-0"
              >
                Primary
              </span>
            {/if}
          </div>

          <!-- Rating -->
          {#if instructor.composite}
            <ScorePopover
              composite={instructor.composite}
              rmp={instructor.rmp}
              bluebook={instructor.bluebook}
              size="xs"
            />
          {/if}

        </div>
      {/each}
    </div>
  {:else}
    <span class="italic text-muted-foreground text-sm">Staff</span>
  {/if}
</div>
