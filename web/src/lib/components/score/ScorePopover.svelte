<script lang="ts">
import type { BlueBookRating, CompositeRating, RmpRating } from "$lib/bindings";
import LazyRichTooltip from "$lib/components/LazyRichTooltip.svelte";
import ScoreBadge from "$lib/components/score/ScoreBadge.svelte";
import { rmpUrl } from "$lib/course";
import { formatNumber } from "$lib/utils";
import { ExternalLink } from "@lucide/svelte";
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
let bbOnly = $derived(hasBb && !hasRmp);
let hasBothSources = $derived(hasBb && hasRmp);
let confident = $derived.by(() => {
  if (hasRmp) return rmp!.isConfident;
  if (hasBb) return bluebook!.isConfident;
  return true;
});
let source = $derived<"composite" | "bluebook" | "rmp">(bbOnly ? "bluebook" : "composite");
</script>

<LazyRichTooltip side="top" sideOffset={6} contentClass="p-0 min-w-52">
  {#if children}
    {@render children()}
  {:else}
    <ScoreBadge score={composite.score} {source} {confident} {size} />
  {/if}
  {#snippet content()}
    <div class="p-3 flex flex-col gap-2.5">
      <!-- Composite headline (only if both sources contribute) -->
      {#if hasBothSources}
        <div class="flex items-center gap-2.5">
          <ScoreBadge score={composite.score} source="composite" {confident} size="sm" />
          <div>
            <div class="text-xs font-medium">Combined Rating</div>
            <div class="text-[10px] text-muted-foreground">
              {formatNumber(composite.totalResponses)} total responses
            </div>
          </div>
        </div>
      {/if}

      <!-- Source breakdown -->
      <div class="flex gap-2">
        {#if hasBb}
          <div
            class="flex-1 rounded border border-border bg-muted/30 px-2.5 py-2 flex flex-col gap-1"
          >
            <div class="text-[10px] font-medium text-muted-foreground uppercase tracking-wide">
              BlueBook
            </div>
            <ScoreBadge
              score={bluebook!.avgInstructorRating}
              source="bluebook"
              confident={bluebook!.isConfident}
              size="xs"
            />
            <div class="text-[10px] text-muted-foreground">
              {formatNumber(bluebook!.totalResponses)} responses
            </div>
          </div>
        {/if}
        {#if hasRmp}
          <div
            class="flex-1 rounded border border-border bg-muted/30 px-2.5 py-2 flex flex-col gap-1"
          >
            <div class="text-[10px] font-medium text-muted-foreground uppercase tracking-wide">
              RateMyProfessors
            </div>
            <ScoreBadge
              score={rmp!.avgRating!}
              source="rmp"
              confident={rmp!.isConfident}
              size="xs"
            />
            <div class="text-[10px] text-muted-foreground">
              {formatNumber(rmp!.numRatings!)} ratings
            </div>
          </div>
        {/if}
      </div>

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
