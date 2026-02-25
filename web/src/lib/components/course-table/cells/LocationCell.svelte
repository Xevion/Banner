<script lang="ts">
import type { CourseResponse } from "$lib/bindings";
import { concernAccentClass, formatLocationTooltip } from "$lib/course";

let { course }: { course: CourseResponse } = $props();

let accentClass = $derived(concernAccentClass(course.instructionalMethod, course.campus));
let locTooltip = $derived(formatLocationTooltip(course));
let locDisplay = $derived(course.primaryLocation);
</script>

<td class="py-2 px-2 whitespace-nowrap">
  {#if locDisplay}
    <span
      class="text-muted-foreground {accentClass ?? ''}"
      class:pl-2={accentClass !== null}
      data-tooltip={locTooltip}
      data-tooltip-delay="200"
    >
      {locDisplay}
    </span>
  {:else}
    <span class="text-xs text-muted-foreground/50">&mdash;</span>
  {/if}
</td>
