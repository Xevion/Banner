<script lang="ts">
import SimpleTooltip from "$lib/components/SimpleTooltip.svelte";

let {
  breakdown = null,
  score = 0,
}: {
  breakdown?: Partial<Record<string, number>> | null;
  score?: number;
} = $props();

const weights: Record<string, number> = {
  name: 0.5,
  department: 0.25,
  uniqueness: 0.15,
  volume: 0.1,
};

const colors: Record<string, string> = {
  name: "bg-blue-500",
  department: "bg-purple-500",
  uniqueness: "bg-amber-500",
  volume: "bg-emerald-500",
};

const labels: Record<string, string> = {
  name: "Name",
  department: "Dept",
  uniqueness: "Unique",
  volume: "Volume",
};

function fmt(v: number): string {
  return (v * 100).toFixed(0);
}

const segments = $derived(
  Object.entries(breakdown ?? {})
    .filter(([, value]) => value != null)
    .map(([key, value]) => ({
      key,
      label: labels[key] ?? key,
      color: colors[key] ?? "bg-primary",
      weight: weights[key] ?? 0,
      raw: value!,
      pct: value! * (weights[key] ?? 0) * 100,
    }))
);

const tooltipText = $derived(
  segments.map((s) => `${s.label}: ${fmt(s.raw)}% \u00d7 ${fmt(s.weight)}%`).join("\n") +
    `\nTotal: ${fmt(score)}%`
);
</script>

<div class="flex items-center gap-2 text-xs">
  <span class="text-muted-foreground shrink-0">Score:</span>
  <div class="bg-muted h-2 flex-1 rounded-full overflow-hidden flex">
    {#each segments as seg (seg.key)}
      <div
        class="{seg.color} h-full transition-all duration-300"
        style="width: {seg.pct}%"
      ></div>
    {/each}
  </div>
  <SimpleTooltip text={tooltipText} side="top">
    <span
      class="tabular-nums font-medium text-foreground cursor-help border-b border-dotted border-muted-foreground/40"
    >
      {fmt(score)}%
    </span>
  </SimpleTooltip>
</div>
