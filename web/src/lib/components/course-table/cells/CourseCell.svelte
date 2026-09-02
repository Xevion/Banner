<script lang="ts">
import type { CourseResponse } from "$lib/bindings";
import { Check, ClipboardCopy } from "@lucide/svelte";
import { getTableContext } from "../context";

let { course }: { course: CourseResponse } = $props();

const { clipboard, subjectMap, maxSubjectLength } = getTableContext();

let subjectDesc = $derived(subjectMap[course.subject]);
let paddedSubject = $derived(course.subject.padStart(maxSubjectLength, " "));
let coursePath = $derived(`/courses/${course.termSlug}/${course.subject}/${course.courseNumber}`);
</script>

<td class="max-w-64 truncate py-2 px-2">
  <div class="flex min-w-0 flex-col gap-px">
    <a
      href={coursePath}
      data-tooltip={subjectDesc
        ? `${subjectDesc} ${course.courseNumber}`
        : `${course.subject} ${course.courseNumber}`}
      data-tooltip-side="bottom"
      data-tooltip-delay="200"
      class="font-mono text-sm font-semibold tracking-[-0.01em] whitespace-pre transition-colors hover:underline"
      >{paddedSubject} {course.courseNumber}{#if course.sequenceNumber}<span
          class="text-muted-foreground">-{course.sequenceNumber}</span
        >{/if}</a
    >

    <a
      href={coursePath}
      class="block truncate text-xs text-muted-foreground transition-colors hover:text-foreground hover:underline"
      data-tooltip={course.title}
      data-tooltip-side="bottom"
      data-tooltip-delay="200">{course.title}</a
    >

    <span class="inline-flex items-center gap-1">
      <a
        href="/courses/{course.termSlug}/{course.crn}"
        class="font-mono text-[11px] text-muted-foreground/70 transition-colors hover:text-foreground hover:underline"
      >
        {course.crn}
      </a>
      <button
        class="inline-flex cursor-copy items-center text-muted-foreground/50 transition-colors duration-150 select-none hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-ring"
        onclick={(e) => clipboard.copy(course.crn, e)}
        onkeydown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            void clipboard.copy(course.crn, e);
          }
        }}
        aria-label="Copy CRN {course.crn} to clipboard"
      >
        {#if clipboard.copiedValue === course.crn}
          <Check class="size-3 text-status-green" />
        {:else}
          <ClipboardCopy class="size-3" />
        {/if}
      </button>
    </span>
  </div>
</td>
