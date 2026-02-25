<script lang="ts">
import BottomSheet from "$lib/components/BottomSheet.svelte";
import { DRAWER_WIDTH } from "$lib/timeline/constants";
import { getSubjectColor } from "$lib/timeline/data";
import { Filter, X } from "@lucide/svelte";

interface Props {
  open: boolean;
  subjects: readonly string[];
  enabledSubjects: Set<string>;
  followEnabled: boolean;
  onToggleSubject: (subject: string) => void;
  onEnableAll: () => void;
  onDisableAll: () => void;
  onResumeFollow: () => void;
}

let {
  open = $bindable(),
  subjects,
  enabledSubjects,
  followEnabled,
  onToggleSubject,
  onEnableAll,
  onDisableAll,
  onResumeFollow,
}: Props = $props();

function onKeyDown(e: KeyboardEvent) {
  if (e.key === "Escape" && open) {
    open = false;
  }
}
</script>

{#snippet followStatus()}
  {#if followEnabled}
    <div
      class="px-2 py-1 rounded-md text-xs font-medium text-center
        bg-green-500/10 text-green-600 dark:text-green-400 border border-green-500/20"
    >
      FOLLOWING
    </div>
  {:else}
    <button
      class="w-full px-2 py-1 rounded-md text-xs font-medium text-center
        bg-muted/80 text-muted-foreground hover:text-foreground
        border border-border/50 transition-colors cursor-pointer"
      onclick={onResumeFollow}
      aria-label="Resume following current time"
    >
      FOLLOW
    </button>
  {/if}
{/snippet}

{#snippet subjectToggles()}
  <div class="flex items-center justify-between mb-2 text-xs text-muted-foreground">
    <span class="uppercase tracking-wider font-medium">Subjects</span>
    <div class="flex gap-1.5">
      <button
        class="hover:text-foreground transition-colors cursor-pointer"
        onclick={onEnableAll}>All</button
      >
      <span class="opacity-40">|</span>
      <button
        class="hover:text-foreground transition-colors cursor-pointer"
        onclick={onDisableAll}>None</button
      >
    </div>
  </div>
  <div class="flex flex-col gap-y-0.5">
    {#each subjects as subject (subject)}
      {@const enabled = enabledSubjects.has(subject)}
      {@const color = getSubjectColor(subject)}
      <button
        class="flex items-center gap-2 w-full px-1.5 py-1 rounded text-xs
          hover:bg-muted/50 transition-colors cursor-pointer text-left"
        onclick={() => onToggleSubject(subject)}
      >
        <span
          class="inline-block w-3 h-3 rounded-sm shrink-0 transition-opacity"
          style="background: {color}; opacity: {enabled ? 1 : 0.2};"
        ></span>
        <span
          class="transition-opacity {enabled
            ? 'text-foreground'
            : 'text-muted-foreground/50'}"
        >
          {subject}
        </span>
      </button>
    {/each}
  </div>
{/snippet}

<svelte:window onkeydown={onKeyDown} />

<!-- Desktop: Filter toggle button -- slides out when drawer opens -->
<div class="hidden md:block">
  <button
    class="absolute right-3 z-50 p-2 rounded-md
      bg-black text-white dark:bg-white dark:text-black
      hover:bg-neutral-800 dark:hover:bg-neutral-200
      border border-black/20 dark:border-white/20
      shadow-md transition-all duration-200 ease-in-out cursor-pointer
      {open ? 'opacity-0 pointer-events-none' : 'opacity-100'}"
    style="top: 20%; transform: translateX({open ? '60px' : '0'});"
    onclick={() => (open = true)}
    aria-label="Open filters"
  >
    <Filter size={18} strokeWidth={2} />
  </button>
</div>

<!-- Desktop: Drawer panel -->
<div class="hidden md:block">
  <div
    class="absolute right-0 z-40 rounded-l-lg shadow-xl transition-transform duration-200 ease-in-out {open ? '' : 'pointer-events-none'}"
    style="top: 20%; width: {DRAWER_WIDTH}px; height: 60%; transform: translateX({open
      ? 0
      : DRAWER_WIDTH}px);"
  >
    <div
      class="h-full flex flex-col bg-background/90 backdrop-blur-md border border-border/40 rounded-l-lg overflow-hidden"
      style="width: {DRAWER_WIDTH}px;"
    >
      <!-- Header -->
      <div class="flex items-center justify-between px-3 py-2.5 border-b border-border/40">
        <span class="text-xs font-semibold text-foreground">Filters</span>
        <button
          class="p-0.5 rounded text-muted-foreground hover:text-foreground transition-colors cursor-pointer"
          onclick={() => (open = false)}
          aria-label="Close filters"
        >
          <X size={14} strokeWidth={2} />
        </button>
      </div>

      <!-- Follow status -->
      <div class="px-3 py-2 border-b border-border/40">
        {@render followStatus()}
      </div>

      <!-- Subject toggles -->
      <div class="flex-1 overflow-y-auto px-3 py-2">
        {@render subjectToggles()}
      </div>
    </div>
  </div>
</div>

<!-- Mobile: Floating filter button -->
<button
  class="fixed right-3 bottom-3 z-50 p-3 rounded-full md:hidden
    bg-black text-white dark:bg-white dark:text-black
    hover:bg-neutral-800 dark:hover:bg-neutral-200
    border border-black/20 dark:border-white/20
    shadow-lg cursor-pointer
    {open ? 'opacity-0 pointer-events-none' : 'opacity-100'}
    transition-opacity duration-200"
  onclick={() => (open = true)}
  aria-label="Open filters"
>
  <Filter size={20} strokeWidth={2} />
</button>

<!-- Mobile: Bottom sheet -->
<div class="md:hidden">
  <BottomSheet bind:open maxHeight="50vh">
    <div class="flex flex-col">
      <!-- Follow status -->
      <div class="px-4 py-2 border-b border-border/40">
        {@render followStatus()}
      </div>

      <!-- Subject toggles -->
      <div class="flex-1 overflow-y-auto px-4 py-2">
        {@render subjectToggles()}
      </div>
    </div>
  </BottomSheet>
</div>
