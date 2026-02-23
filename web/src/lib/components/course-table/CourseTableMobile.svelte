<script lang="ts">
import type { CourseResponse } from "$lib/bindings";
import CourseCard from "$lib/components/CourseCard.svelte";
import EmptyState from "./EmptyState.svelte";
import { buildCardSkeletonHtml } from "./skeletons";

let {
  courses,
  loading,
  skeletonRowCount,
  expandedCrn,
  onToggle,
}: {
  courses: CourseResponse[];
  loading: boolean;
  skeletonRowCount: number;
  expandedCrn: string | null;
  onToggle: (crn: string) => void;
} = $props();
</script>

<div class="flex flex-col gap-2 sm:hidden">
  {#if loading && courses.length === 0}
    <!-- eslint-disable-next-line svelte/no-at-html-tags -- Static skeleton markup, no user input -->
    {@html buildCardSkeletonHtml(skeletonRowCount)}
  {:else if courses.length === 0 && !loading}
    <EmptyState />
  {:else}
    {#each courses as course (course.crn)}
      <div class="transition-opacity duration-200 {loading ? 'opacity-45 pointer-events-none' : ''}">
        <CourseCard
          {course}
          expanded={expandedCrn === course.crn}
          onToggle={() => onToggle(course.crn)}
        />
      </div>
    {/each}
  {/if}
</div>
