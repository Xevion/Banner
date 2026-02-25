<script lang="ts">
import type { CourseResponse } from "$lib/bindings";
import LazyRichTooltip from "$lib/components/LazyRichTooltip.svelte";
import { abbreviateInstructor, getPrimaryInstructor, ratingStyle, rmpUrl } from "$lib/course";
import { themeStore } from "$lib/stores/theme.svelte";
import { formatNumber } from "$lib/utils";
import { BookOpen, ExternalLink, Star, Triangle } from "@lucide/svelte";

let { course }: { course: CourseResponse } = $props();

let primary = $derived(getPrimaryInstructor(course.instructors, course.primaryInstructorId));
let display = $derived(primary ? abbreviateInstructor(primary.displayName) : "Staff");
let commaIdx = $derived(display.indexOf(", "));
let profileUrl = $derived(primary?.slug ? `/instructors/${primary.slug}` : null);

let ratingData = $derived.by(() => {
  if (!primary) return null;
  const comp = primary.composite;
  if (!comp) return null;

  const hasRmp = primary.rmp?.avgRating != null && primary.rmp?.numRatings != null;
  const hasBb = primary.bluebook != null;
  const isConfident = hasRmp
    ? primary.rmp!.isConfident
    : hasBb
      ? primary.bluebook!.isConfident
      : true;

  return {
    score: comp.score,
    totalResponses: comp.totalResponses,
    isConfident,
    rmp: hasRmp
      ? {
          rating: primary.rmp!.avgRating!,
          count: primary.rmp!.numRatings!,
          legacyId: primary.rmp!.legacyId,
        }
      : null,
    bb: hasBb
      ? { rating: primary.bluebook!.avgInstructorRating, count: primary.bluebook!.totalResponses }
      : null,
    bbOnly: hasBb && !hasRmp,
  };
});
</script>

<td class="py-2 px-2 whitespace-nowrap">
  {#if display === "Staff"}
    <span class="text-xs text-muted-foreground/60 uppercase select-none">Staff</span>
  {:else}
    {#if profileUrl}
      <a
        href={profileUrl}
        data-tooltip={primary?.displayName ?? "Staff"}
        data-tooltip-side="bottom"
        data-tooltip-delay="200"
        class="hover:underline"
      >
        {#if commaIdx !== -1}
          <span
            >{display.slice(0, commaIdx)},
            <span class="text-muted-foreground"
              >{display.slice(commaIdx + 1)}</span
            ></span
          >
        {:else}
          <span>{display}</span>
        {/if}
      </a>
    {:else}
      <span
        data-tooltip={primary?.displayName ?? "Staff"}
        data-tooltip-side="bottom"
        data-tooltip-delay="200"
      >
        {#if commaIdx !== -1}
          <span
            >{display.slice(0, commaIdx)},
            <span class="text-muted-foreground"
              >{display.slice(commaIdx + 1)}</span
            ></span
          >
        {:else}
          <span>{display}</span>
        {/if}
      </span>
    {/if}
  {/if}
  {#if ratingData}
    {@const lowConfidence = !ratingData.isConfident}
    <LazyRichTooltip side="bottom" sideOffset={6} contentClass="px-2.5 py-1.5">
      <span
        class="ml-1 text-xs font-medium inline-flex items-center gap-0.5 select-none"
        style={ratingStyle(ratingData.score, themeStore.isDark)}
      >
        {ratingData.score.toFixed(1)}
        {#if lowConfidence}
          <Triangle class="size-2 fill-current" />
        {:else if ratingData.bbOnly}
          <BookOpen class="size-2.5 fill-current" />
        {:else}
          <Star class="size-2.5 fill-current" />
        {/if}
      </span>
      {#snippet content()}
        <span class="inline-flex items-center gap-1.5 text-xs flex-wrap">
          {#if ratingData.bb && ratingData.rmp}
            BlueBook: {ratingData.bb.rating.toFixed(1)}/5 ({formatNumber(ratingData.bb.count)})
            &middot;
            RMP: {ratingData.rmp.rating.toFixed(1)}/5 ({formatNumber(ratingData.rmp.count)})
            &middot;
            Combined: {ratingData.score.toFixed(1)}/5
          {:else if ratingData.bb}
            {ratingData.bb.rating.toFixed(1)}/5 &middot; {formatNumber(ratingData.bb.count)} responses (BlueBook)
          {:else if ratingData.rmp}
            {ratingData.rmp.rating.toFixed(1)}/5 &middot; {formatNumber(ratingData.rmp.count)}
            ratings
            {#if !ratingData.isConfident}
              (low)
            {/if}
          {/if}
          {#if ratingData.rmp?.legacyId != null}
            &middot;
            <a
              href={rmpUrl(ratingData.rmp.legacyId)}
              target="_blank"
              rel="noopener"
              class="inline-flex items-center gap-0.5 text-muted-foreground hover:text-foreground transition-colors"
            >
              RMP
              <ExternalLink class="size-3" />
            </a>
          {/if}
        </span>
      {/snippet}
    </LazyRichTooltip>
  {/if}
</td>
