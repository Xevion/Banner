<script lang="ts">
import type { BlueBookRating, CompositeRating, RmpRating } from "$lib/bindings";
import LazyRichTooltip from "$lib/components/LazyRichTooltip.svelte";
import ScoreBadge from "$lib/components/score/ScoreBadge.svelte";
import { ratingColor, rmpUrl } from "$lib/course";
import { themeStore } from "$lib/stores/theme.svelte";
import { formatNumber } from "$lib/utils";
import { BookOpen, ExternalLink, Star } from "@lucide/svelte";
import type { Snippet } from "svelte";

let {
  composite,
  rmp = null,
  bluebook = null,
  size = "xs",
  children,
}: {
  composite: CompositeRating;
  rmp?: RmpRating | null;
  bluebook?: BlueBookRating | null;
  size?: "xs" | "sm";
  children?: Snippet;
} = $props();

let hasRmp = $derived(rmp?.avgRating != null && rmp?.numRatings != null);
let hasBb = $derived(bluebook != null);
let hasBothSources = $derived(hasBb && hasRmp);

let ciHalf = $derived(((composite.ciUpper - composite.ciLower) / 2).toFixed(1));
let ciBarLeft = $derived(((composite.ciLower - 1) / 4) * 100);
let ciBarWidth = $derived(((composite.ciUpper - composite.ciLower) / 4) * 100);
let ciDotLeft = $derived(((composite.displayScore - 1) / 4) * 100);
let color = $derived(ratingColor(composite.displayScore, themeStore.isDark));
let confidencePct = $derived((composite.confidence * 100).toFixed(0));

let pillSource = $derived<"composite" | "bluebook" | "rmp">(
  hasBothSources ? "composite" : hasBb ? "bluebook" : "rmp"
);
let headerLabel = $derived(
  hasBothSources ? "Combined Rating" : hasBb ? "BlueBook" : "RateMyProfessors"
);
</script>

<LazyRichTooltip side="top" sideOffset={6} contentClass="p-0 min-w-52">
  {#if children}
    {@render children()}
  {:else}
    <ScoreBadge
      score={composite.displayScore}
      source={pillSource}
      confidence={composite.confidence}
      {size}
    />
  {/if}
  {#snippet content()}
    <div class="p-3 flex flex-col gap-2.5">
      <!-- Composite header -->
      <div class="flex items-center gap-2.5">
        <ScoreBadge
          score={composite.displayScore}
          source={pillSource}
          confidence={composite.confidence}
          size="sm"
        />
        <div>
          <div class="text-xs font-medium">{headerLabel}</div>
          <div class="text-[10px] text-muted-foreground tabular-nums">
            {composite.displayScore.toFixed(2)} ± {ciHalf} · {confidencePct}% confidence
          </div>
        </div>
      </div>

      <!-- Mini CI bar -->
      <div class="relative h-1.5 w-full rounded-full bg-muted/40 overflow-hidden">
        <div
          class="absolute inset-y-0 rounded-full opacity-40"
          style="left: {ciBarLeft}%; width: {ciBarWidth}%; background: {color}"
        ></div>
        <div
          class="absolute top-1/2 -translate-y-1/2 size-2 rounded-full"
          style="left: calc({ciDotLeft}% - 0.25rem); background: {color}"
        ></div>
      </div>

      <!-- Total responses -->
      <div class="text-[10px] text-muted-foreground">
        {formatNumber(composite.totalResponses)} total responses
      </div>

      <!-- Stacked source rows -->
      {#if hasBothSources}
        <hr class="border-dashed border-border" />
        <!-- BlueBook row -->
        <div class="flex items-center gap-1.5">
          <BookOpen class="size-3 text-muted-foreground shrink-0" />
          <span class="flex-1 text-[10px]">BlueBook</span>
          <span
            class="text-[10px] tabular-nums font-medium"
            style="color: {ratingColor(bluebook!.avgInstructorRating, themeStore.isDark)}"
          >
            {bluebook!.avgInstructorRating.toFixed(1)}
          </span>
          <span class="text-[10px] text-muted-foreground">
            {formatNumber(bluebook!.totalResponses)}
          </span>
        </div>
        <!-- RMP row -->
        <div class="flex items-center gap-1.5">
          <Star class="size-3 text-muted-foreground shrink-0" />
          <span class="flex-1 text-[10px]">RateMyProfessors</span>
          <span
            class="text-[10px] tabular-nums font-medium"
            style="color: {ratingColor(rmp!.avgRating!, themeStore.isDark)}"
          >
            {rmp!.avgRating!.toFixed(1)}
          </span>
          <span class="text-[10px] text-muted-foreground">
            {formatNumber(rmp!.numRatings!)}
          </span>
        </div>
      {/if}

      <!-- RMP external link -->
      {#if rmp?.legacyId != null}
        <div class="border-t border-dashed border-border pt-2">
          <a
            href={rmpUrl(rmp.legacyId)}
            target="_blank"
            rel="noopener"
            class="inline-flex items-center gap-1 text-[11px] text-muted-foreground hover:text-foreground transition-colors"
          >
            View on RateMyProfessors
            <ExternalLink class="size-3" />
          </a>
        </div>
      {/if}
    </div>
  {/snippet}
</LazyRichTooltip>
