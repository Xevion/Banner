<script lang="ts">
import { getSubjectColor } from "$lib/timeline/data";
import type { TimeSlot } from "$lib/timeline/types";
import { enabledTotalClasses } from "$lib/timeline/viewport";
import { timeFormat } from "d3-time-format";

interface Props {
  visible: boolean;
  x: number;
  y: number;
  slot: TimeSlot | null;
  activeSubjects: readonly string[];
}

let { visible, x, y, slot, activeSubjects }: Props = $props();

const fmtTime = timeFormat("%-I:%M %p");
</script>

{#if visible && slot}
    {@const total = enabledTotalClasses(slot, activeSubjects)}
    <div
        class="pointer-events-none fixed z-50 rounded-lg border border-border/60 bg-background/95
      backdrop-blur-sm shadow-lg px-3 py-2 text-xs min-w-35"
        style="left: {x + 12}px; top: {y - 10}px; transform: translateY(-100%);"
    >
        <div class="font-semibold text-foreground mb-1.5">
            {fmtTime(slot.time)}
        </div>
        <div class="flex flex-col gap-y-0.5">
            {#each activeSubjects as subject (subject)}
                {@const count = slot.subjects[subject] ?? 0}
                {#if count > 0}
                    <div class="flex items-center justify-between gap-3">
                        <div class="flex items-center gap-1.5">
                            <span
                                class="inline-block w-2 h-2 rounded-sm"
                                style="background: {getSubjectColor(subject)}"
                            ></span>
                            <span class="text-muted-foreground">{subject}</span>
                        </div>
                        <span class="font-medium tabular-nums">{count}</span>
                    </div>
                {/if}
            {/each}
        </div>
        <div
            class="mt-1.5 pt-1.5 border-t border-border/40 flex justify-between font-medium"
        >
            <span>Total</span>
            <span class="tabular-nums">{total}</span>
        </div>
    </div>
{/if}
