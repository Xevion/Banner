<script lang="ts">
import type { PublicBlueBookSummary, PublicRmpSummary } from "$lib/bindings";
import StatItem from "$lib/components/score/StatItem.svelte";
import { ratingColor, rmpUrl, scoreBadgeStyle } from "$lib/course";
import { themeStore } from "$lib/stores/theme.svelte";
import { formatNumber } from "$lib/utils";
import { BookOpen, ExternalLink, Star } from "@lucide/svelte";

let {
  source,
  bluebook = undefined,
  rmp = undefined,
}: {
  source: "bluebook" | "rmp";
  bluebook?: PublicBlueBookSummary;
  rmp?: PublicRmpSummary;
} = $props();
</script>

<div class="rounded-lg border border-border bg-card p-5">
  {#if source === "bluebook" && bluebook}
    {@const bb = bluebook}
    <!-- BlueBook Card -->
    <div class="flex items-center gap-2 mb-3">
      <BookOpen class="size-4 text-muted-foreground" />
      <h3 class="text-sm font-semibold">BlueBook</h3>
    </div>
    <div class="flex items-center gap-6 flex-wrap">
      <div class="text-center">
        <div
          class="text-2xl font-bold"
          style={scoreBadgeStyle(bb.avgInstructorRating, themeStore.isDark)}
        >
          {bb.avgInstructorRating.toFixed(1)}
        </div>
        <!-- Proportional bar -->
        <div class="w-full h-1 rounded-full bg-muted mt-1">
          <div
            class="h-full rounded-full"
            style="width: {(bb.avgInstructorRating / 5) * 100}%; background-color: {ratingColor(bb.avgInstructorRating, themeStore.isDark)}"
          ></div>
        </div>
        <div class="text-xs text-muted-foreground mt-1">Instructor</div>
      </div>
      {#if bb.avgCourseRating != null}
        <StatItem value={bb.avgCourseRating.toFixed(1)} label="Course" />
      {/if}
      <StatItem value={formatNumber(bb.totalResponses)} label="Responses" />
      <StatItem value={String(bb.evalCount)} label="Evaluations" />
    </div>
  {:else if source === "rmp" && rmp}
    {@const r = rmp}
    <!-- RMP Card -->
    <div class="flex items-center justify-between mb-3">
      <div class="flex items-center gap-2">
        <Star class="size-4 text-muted-foreground" />
        <h3 class="text-sm font-semibold">RateMyProfessors</h3>
      </div>
      <a
        href={rmpUrl(r.legacyId)}
        target="_blank"
        rel="noopener"
        class="inline-flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
      >
        View
        <ExternalLink class="size-3" />
      </a>
    </div>
    <div class="flex items-center gap-6 flex-wrap">
      {#if r.avgRating != null}
        {@const rating = r.avgRating}
        <div class="text-center">
          <div
            class="text-2xl font-bold"
            style={scoreBadgeStyle(rating, themeStore.isDark)}
          >
            {rating.toFixed(1)}
          </div>
          <!-- Proportional bar -->
          <div class="w-full h-1 rounded-full bg-muted mt-1">
            <div
              class="h-full rounded-full"
              style="width: {(rating / 5) * 100}%; background-color: {ratingColor(rating, themeStore.isDark)}"
            ></div>
          </div>
          <div class="text-xs text-muted-foreground mt-1">Overall</div>
        </div>
        {#if r.avgDifficulty != null}
          <StatItem value={r.avgDifficulty.toFixed(1)} label="Difficulty" />
        {/if}
        {#if r.wouldTakeAgainPct != null}
          <StatItem value="{Math.round(r.wouldTakeAgainPct)}%" label="Take Again" />
        {/if}
        {#if r.numRatings != null}
          <StatItem value={formatNumber(r.numRatings)} label="Ratings" />
        {/if}
      {:else}
        <span class="text-sm text-muted-foreground">No ratings yet</span>
      {/if}
    </div>
  {/if}
</div>
