<script lang="ts" generics="S extends Record<string, number>">
import type { ProgressSegment } from "$lib/ui";

let {
  stats,
  segments,
  total,
}: {
  stats: S;
  segments: ProgressSegment<S>[];
  total: number;
} = $props();

const denom = $derived(total || 1);
</script>

<div class="mb-6">
  <div class="bg-muted h-2 rounded-full overflow-hidden flex">
    {#each segments as seg (seg.stat)}
      {@const value = stats[seg.stat] ?? 0}
      <div
        class="{seg.color} h-full transition-all duration-500"
        style="width: {(value / denom) * 100}%"
        title="{seg.label}: {value}"
      ></div>
    {/each}
  </div>
</div>
