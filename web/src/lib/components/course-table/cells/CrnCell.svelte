<script lang="ts">
import type { CourseResponse } from "$lib/bindings";
import { Check, ClipboardCopy } from "@lucide/svelte";
import { getTableContext } from "../context";

let { course }: { course: CourseResponse } = $props();

const { clipboard } = getTableContext();
</script>

<td class="py-2 px-2 relative">
  <span class="inline-flex items-center gap-1">
    <a
      href="/courses/{course.termSlug}/{course.crn}"
      class="relative inline-flex items-center rounded-full px-2 py-0.5 border border-border/50 bg-muted/20 hover:bg-muted/40 hover:border-foreground/30 transition-colors duration-150 font-mono text-xs text-muted-foreground/70 hover:text-foreground"
    >
      {course.crn}
    </a>
    <button
      class="inline-flex items-center text-muted-foreground/50 hover:text-foreground transition-colors duration-150 cursor-copy select-none focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-ring"
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
        <Check class="size-3 text-green-500" />
      {:else}
        <ClipboardCopy class="size-3" />
      {/if}
    </button>
  </span>
</td>
