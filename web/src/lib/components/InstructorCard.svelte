<script lang="ts">
import type { BlueBookBrief, InstructorRating, RmpBrief } from "$lib/bindings";
import ScoreBadge from "$lib/components/score/ScoreBadge.svelte";
import ScorePopover from "$lib/components/score/ScorePopover.svelte";
import { cn } from "$lib/utils";
import { Mail } from "@lucide/svelte";

let {
  name,
  slug = null,
  variant = "panel",
  email = null,
  subjects = [],
  subjectLabel,
  maxSubjects = 4,
  rating = null,
  rmp = null,
  bluebook = null,
  badge = null,
}: {
  /** Display-ready name. Callers decide between Banner's "Last, First" and formatInstructorName. */
  name: string;
  /** Null renders plain text, for instructors with no public profile. */
  slug?: string | null;
  variant?: "grid" | "panel" | "compact";
  email?: string | null;
  subjects?: string[];
  /** Maps a subject code to a longer label; codes render bare without it. */
  subjectLabel?: (code: string) => string;
  maxSubjects?: number;
  rating?: InstructorRating | null;
  rmp?: RmpBrief | null;
  bluebook?: BlueBookBrief | null;
  badge?: string | null;
} = $props();

const rowClasses = {
  panel: "flex items-center gap-3 border border-border rounded-lg px-4 py-3 bg-card",
  compact:
    "flex items-center flex-wrap gap-x-3 gap-y-1 border border-border rounded-md px-3 py-1.5 bg-card",
} as const;

const nameClass = "font-medium text-sm text-foreground truncate";

const shownSubjects = $derived(subjects.slice(0, maxSubjects));
const hiddenSubjectCount = $derived(Math.max(0, subjects.length - maxSubjects));

function label(code: string): string {
  return subjectLabel ? subjectLabel(code) : code;
}
</script>

{#snippet subjectChips(gap: string)}
  {#if subjects.length > 0}
    <div class={cn("relative flex flex-wrap gap-1", gap)}>
      {#each shownSubjects as subject (subject)}
        <a
          href="/subjects/{subject}"
          class="relative z-10 inline-block px-1.5 py-0.5 text-[10px] font-medium rounded
                 bg-muted text-muted-foreground truncate max-w-32
                 hover:bg-muted/80 hover:text-foreground transition-colors"
        >
          {label(subject)}
        </a>
      {/each}
      {#if hiddenSubjectCount > 0}
        <span class="text-[10px] text-muted-foreground self-center">+{hiddenSubjectCount}</span>
      {/if}
    </div>
  {/if}
{/snippet}

{#if variant === "grid"}
  <div
    class="relative rounded-lg border border-border bg-card p-4
           hover:border-foreground/20 hover:shadow-sm transition-all"
  >
    {#if slug}
      <a
        href="/instructors/{slug}"
        class="absolute inset-0 rounded-lg
               focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring"
        aria-label={name}
      ></a>
    {/if}
    <div class="flex items-start justify-between gap-2">
      <div class="min-w-0 flex-1">
        <h2 class="font-semibold text-sm truncate">{name}</h2>
        {#if email}
          <div class="flex items-center gap-1 mt-0.5 text-xs text-muted-foreground">
            <Mail class="size-3 shrink-0" />
            <span class="truncate">{email}</span>
          </div>
        {/if}
      </div>
      {#if rating}
        <!-- A badge, not the popover: the card-wide overlay link owns pointer events here. -->
        <div class="shrink-0">
          <ScoreBadge score={rating.score} confidence={rating.confidence} size="sm" />
        </div>
      {/if}
    </div>
    {@render subjectChips("mt-2.5")}
  </div>
{:else}
  <div class={rowClasses[variant]}>
    <div class={cn("flex items-center gap-2 min-w-0", variant === "panel" && "flex-1")}>
      {#if slug}
        <a href="/instructors/{slug}" class={cn(nameClass, "hover:underline")}>{name}</a>
      {:else}
        <span class={nameClass}>{name}</span>
      {/if}
      {#if badge}
        <span
          class="text-[10px] font-medium text-muted-foreground bg-muted rounded px-1.5 py-0.5 shrink-0"
        >
          {badge}
        </span>
      {/if}
    </div>
    {#if rating}
      <ScorePopover {rating} {rmp} {bluebook} size="xs" />
    {/if}
    {@render subjectChips("")}
  </div>
{/if}
