<script lang="ts">
import type { ScoreBreakdown as ScoreBreakdownType } from "$lib/bindings";
import SimpleTooltip from "$lib/components/SimpleTooltip.svelte";

let {
  breakdown = null,
  score = 0,
}: {
  breakdown?: ScoreBreakdownType | null;
  score?: number;
} = $props();

/** Signals used in the composite score with their actual weights. */
const weights: Record<string, number> = {
  name: 0.5,
  subject: 0.3,
  uniqueness: 0.15,
  volume: 0.05,
};

const colors: Record<string, string> = {
  name: "bg-blue-500",
  subject: "bg-purple-500",
  uniqueness: "bg-amber-500",
  volume: "bg-emerald-500",
};

const labels: Record<string, string> = {
  name: "Name",
  subject: "Subject",
  uniqueness: "Unique",
  volume: "Volume",
};

function fmt(v: number): string {
  return (v * 100).toFixed(0);
}

/** Only include the four composite signals (skip raw department/review_courses). */
const compositeKeys: (keyof ScoreBreakdownType)[] = ["name", "subject", "uniqueness", "volume"];

const segments = $derived(
  compositeKeys
    .filter((key) => breakdown?.[key] != null)
    .map((key) => ({
      key,
      label: labels[key] ?? key,
      color: colors[key] ?? "bg-primary",
      weight: weights[key] ?? 0,
      raw: breakdown![key],
      pct: breakdown![key] * (weights[key] ?? 0) * 100,
    }))
);

const tooltipText = $derived.by(() => {
  const lines = segments.map((s) => `${s.label}: ${fmt(s.raw)}% \u00d7 ${fmt(s.weight)}%`);

  // Show department and review_courses as sub-detail under Subject
  const dept = breakdown?.department;
  const reviews = breakdown?.reviewCourses;
  if (dept != null || reviews != null) {
    const parts: string[] = [];
    if (dept != null) parts.push(`dept ${fmt(dept)}%`);
    if (reviews != null) parts.push(`reviews ${fmt(reviews)}%`);
    lines.push(`  \u2514 ${parts.join(", ")}`);
  }

  lines.push(`Total: ${fmt(score)}%`);
  return lines.join("\n");
});
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
