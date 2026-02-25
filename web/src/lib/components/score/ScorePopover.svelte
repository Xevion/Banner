<script lang="ts">
import type { BlueBookBrief, InstructorRating, RmpBrief } from "$lib/bindings";
import LazyRichTooltip from "$lib/components/LazyRichTooltip.svelte";
import ScoreBadge from "$lib/components/score/ScoreBadge.svelte";
import { ratingColor, rmpUrl } from "$lib/course";
import { themeStore } from "$lib/stores/theme.svelte";
import { formatNumber } from "$lib/utils";
import { BookOpen, ExternalLink, Star } from "@lucide/svelte";
import type { Snippet } from "svelte";

let {
  rating,
  rmp = null,
  bluebook = null,
  size = "xs",
  children,
}: {
  rating: InstructorRating;
  rmp?: RmpBrief | null;
  bluebook?: BlueBookBrief | null;
  size?: "xs" | "sm";
  children?: Snippet;
} = $props();

let hasRmp = $derived(rmp?.avgRating != null && rmp?.numRatings != null);
let hasBb = $derived(bluebook != null);

let ciHalf = $derived(((rating.ciUpper - rating.ciLower) / 2).toFixed(1));
let ciBarLeft = $derived(((rating.ciLower - 1) / 4) * 100);
let ciBarWidth = $derived(((rating.ciUpper - rating.ciLower) / 4) * 100);
let ciDotLeft = $derived(((rating.score - 1) / 4) * 100);
let color = $derived(ratingColor(rating.score, themeStore.isDark));
let confidencePct = $derived((rating.confidence * 100).toFixed(0));
</script>

<LazyRichTooltip side="top" sideOffset={6} contentClass="p-0 min-w-52">
  {#if children}
    {@render children()}
  {:else}
    <ScoreBadge
      score={rating.score}
      confidence={rating.confidence}
      {size}
    />
  {/if}
  {#snippet content()}
    <div class="p-3 flex flex-col gap-2.5">
      <!-- Rating header -->
      <div class="flex items-center gap-2.5">
        <ScoreBadge
          score={rating.score}
          confidence={rating.confidence}
          size="sm"
        />
        <div>
          <div class="text-xs font-medium">Rating</div>
          <div class="text-[10px] text-muted-foreground tabular-nums">
            {rating.score.toFixed(2)} ± {ciHalf} · {confidencePct}% confidence
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
        {formatNumber(rating.totalResponses)} total responses
      </div>

      <!-- Stacked source rows -->
      {#if hasBb || hasRmp}
        <hr class="border-dashed border-border" />
        {#if hasBb}
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
        {/if}
        {#if hasRmp}
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
