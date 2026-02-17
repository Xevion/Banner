<script lang="ts">
import type { CourseResponse } from "$lib/bindings";
import { fade } from "svelte/transition";
import { getTableContext } from "../context";

let { course }: { course: CourseResponse } = $props();

const { clipboard } = getTableContext();
</script>

<td class="py-2 px-2 relative">
  <button
    class="relative inline-flex items-center rounded-full px-2 py-0.5 border border-border/50 bg-muted/20 hover:bg-muted/40 hover:border-foreground/30 transition-colors duration-150 cursor-copy select-none focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-ring font-mono text-xs text-muted-foreground/70"
    onclick={(e) => clipboard.copy(course.crn, e)}
    onkeydown={(e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        void clipboard.copy(course.crn, e);
      }
    }}
    aria-label="Copy CRN {course.crn} to clipboard"
  >
    {course.crn}
    {#if clipboard.copiedValue === course.crn}
      <span
        class="absolute -top-8 left-1/2 -translate-x-1/2 whitespace-nowrap text-xs px-2 py-1 rounded-md bg-green-500/10 border border-green-500/20 text-green-700 dark:text-green-300 pointer-events-none z-10"
        in:fade={{ duration: 100 }}
        out:fade={{ duration: 200 }}
      >
        Copied!
      </span>
    {/if}
  </button>
</td>
