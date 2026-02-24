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
      {@const pct = (stats[seg.stat] / denom) * 100}
      <div
        class="{seg.color} h-full transition-all duration-500"
        style="width: {pct}%"
        title="{seg.label}: {stats[seg.stat]}"
      ></div>
    {/each}
  </div>
</div>
