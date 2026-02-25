<script lang="ts">
import { scoreBadgeStyle } from "$lib/course";
import { themeStore } from "$lib/stores/theme.svelte";
import { cn } from "$lib/utils";
import { BookOpen, Star, Triangle } from "@lucide/svelte";

let {
  score,
  source = "composite",
  confident = true,
  size = "xs",
}: {
  score: number;
  source?: "composite" | "bluebook" | "rmp";
  confident?: boolean;
  size?: "xs" | "sm" | "lg";
} = $props();

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
    !confident && "border border-dashed border-current/30",
  )}
  style={scoreBadgeStyle(score, themeStore.isDark)}
>
  {score.toFixed(1)}
  {#if !confident}
    <Triangle class={cn(iconSizes[size], "fill-current")} />
  {:else if source === "bluebook"}
    <BookOpen class={cn(iconSizes[size], "fill-current")} />
  {:else}
    <Star class={cn(iconSizes[size], "fill-current")} />
  {/if}
</span>
