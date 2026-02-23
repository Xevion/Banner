<script lang="ts">
import type { CourseResponse } from "$lib/bindings";
import { abbreviateInstructor, getPrimaryInstructor, ratingStyle, rmpUrl } from "$lib/course";
import { themeStore } from "$lib/stores/theme.svelte";
import { formatNumber } from "$lib/utils";
import { ExternalLink, Star, Triangle } from "@lucide/svelte";
import LazyRichTooltip from "$lib/components/LazyRichTooltip.svelte";

let { course }: { course: CourseResponse } = $props();

let primary = $derived(getPrimaryInstructor(course.instructors, course.primaryInstructorId));
let display = $derived(primary ? abbreviateInstructor(primary.displayName) : "Staff");
let commaIdx = $derived(display.indexOf(", "));
let profileUrl = $derived(primary?.slug ? `/instructors/${primary.slug}` : null);
let ratingData = $derived(
  primary?.rmp != null
    ? {
        rating: primary.rmp.avgRating,
        count: primary.rmp.numRatings,
        legacyId: primary.rmp.legacyId,
        isConfident: primary.rmp.isConfident,
      }
    : null
);
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
        style={ratingStyle(ratingData.rating, themeStore.isDark)}
      >
        {ratingData.rating.toFixed(1)}
        {#if lowConfidence}
          <Triangle class="size-2 fill-current" />
        {:else}
          <Star class="size-2.5 fill-current" />
        {/if}
      </span>
      {#snippet content()}
        <span class="inline-flex items-center gap-1.5 text-xs">
          {ratingData.rating.toFixed(1)}/5 · {formatNumber(ratingData.count)}
          ratings
          {#if !ratingData.isConfident}
            (low)
          {/if}
          {#if ratingData.legacyId != null}
            ·
            <a
              href={rmpUrl(ratingData.legacyId)}
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
