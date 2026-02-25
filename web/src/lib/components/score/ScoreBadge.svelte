<script lang="ts">
import { scoreBadgeStyle } from "$lib/course";
import { themeStore } from "$lib/stores/theme.svelte";
import { cn } from "$lib/utils";
import { Star, Triangle } from "@lucide/svelte";

let {
  score,
  confidence = 1,
  size = "xs",
}: {
  score: number;
  confidence?: number;
  size?: "xs" | "sm" | "lg";
} = $props();

// >= 0.5: well-sampled (7+ RMP or 10+ BB responses) -- solid border, source icon
// [0.3, 0.5): sparse data -- dashed border, source icon
// < 0.3: very sparse (e.g. 1 BB response) -- dashed border, Triangle overrides source
const tier = $derived(confidence >= 0.5 ? "high" : confidence >= 0.3 ? "medium" : "low");

const sizeClasses = {
  xs: "text-xs px-1.5 py-0.5 gap-0.5",
  sm: "text-sm px-2 py-0.5 gap-1",
  lg: "text-3xl px-3 py-1 gap-1.5",
} as const;

const iconSizes = {
  xs: "size-2.5",
  sm: "size-3",
  lg: "size-5",
} as const;
</script>

<span
  class={cn(
    "inline-flex items-center font-semibold rounded-md select-none",
    sizeClasses[size],
    tier !== "high" && "border border-dashed border-current/30",
  )}
  style={scoreBadgeStyle(score, themeStore.isDark)}
>
  {score.toFixed(1)}
  {#if tier === "low"}
    <Triangle class={cn(iconSizes[size], "fill-current")} />
  {:else}
    <Star class={cn(iconSizes[size], "fill-current")} />
  {/if}
</span>
