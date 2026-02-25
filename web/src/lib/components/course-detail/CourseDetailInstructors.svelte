<script lang="ts">
import type { CourseResponse } from "$lib/bindings";
import ScorePopover from "$lib/components/score/ScorePopover.svelte";
import { useClipboard } from "$lib/composables/useClipboard.svelte";
import { formatInstructorName, rmpUrl } from "$lib/course";
import { Check, Copy, ExternalLink } from "@lucide/svelte";

let { course }: { course: CourseResponse } = $props();

const clipboard = useClipboard();
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
            <span class="font-medium text-sm text-foreground truncate">
              {formatInstructorName(instructor)}
            </span>
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

          <!-- Email + RMP link -->
          <div class="flex items-center gap-3 text-xs text-muted-foreground">
            {#if instructor.email}
              <button
                onclick={(e) => clipboard.copy(instructor.email!, e)}
                class="inline-flex items-center gap-1 hover:text-foreground transition-colors cursor-pointer"
              >
                {#if clipboard.copiedValue === instructor.email}
                  <Check class="size-3" />
                  <span>Copied!</span>
                {:else}
                  <Copy class="size-3" />
                  <span>{instructor.email}</span>
                {/if}
              </button>
            {/if}
            {#if instructor.rmp?.legacyId != null}
              <a
                href={rmpUrl(instructor.rmp.legacyId)}
                target="_blank"
                rel="noopener"
                class="inline-flex items-center gap-1 hover:text-foreground transition-colors"
              >
                <ExternalLink class="size-3" />
                <span>RMP</span>
              </a>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {:else}
    <span class="italic text-muted-foreground text-sm">Staff</span>
  {/if}
</div>
