<script lang="ts" generics="S extends Record<string, number>">
import type { FilterCard } from "$lib/ui";

let {
  stats,
  cards,
  activeFilter,
  onSelect,
}: {
  stats: S;
  cards: FilterCard<S>[];
  activeFilter: string | undefined;
  onSelect: (value: string | undefined) => void;
} = $props();
</script>

<div class="mb-4 grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-5 gap-3">
  {#each cards as card (card.value)}
    <button
      onclick={() => onSelect(card.value)}
      class="bg-card border-border rounded-lg border p-3 text-left transition-all duration-200
             cursor-pointer hover:shadow-sm hover:border-border/80
             {activeFilter === card.value ? `ring-2 ${card.ringColor} shadow-sm` : ''}"
    >
      <div class="text-xs {card.textColor}">{card.label}</div>
      <div class="text-lg font-semibold tabular-nums">{stats[card.stat]}</div>
    </button>
  {/each}
</div>
