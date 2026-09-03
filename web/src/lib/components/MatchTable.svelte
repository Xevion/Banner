<script lang="ts" generics="T, D extends object">
import type { MatchColumn } from "$lib/ui";
import { ChevronRight } from "@lucide/svelte";
import type { Snippet } from "svelte";
import { slide } from "svelte/transition";

let {
  columns,
  rows,
  getId,
  expandedId,
  isStale,
  isHighlighted,
  detail,
  detailLoading,
  detailError,
  onToggle,
  cells,
  actions,
  detailPanel,
}: {
  /** Header labels, excluding the trailing actions column this component owns. */
  columns: MatchColumn[];
  rows: T[];
  getId: (row: T) => number;
  expandedId: number | null;
  /** Dims rows whose status no longer matches the active filter. */
  isStale: (row: T) => boolean;
  isHighlighted: (id: number) => boolean;
  detail: D | null;
  detailLoading: boolean;
  detailError: string | null;
  onToggle: (id: number) => void;
  cells: Snippet<[T]>;
  actions?: Snippet<[T]>;
  detailPanel: Snippet<[D]>;
} = $props();
</script>

<div class="bg-card border-border overflow-hidden rounded-lg border">
  <table class="w-full text-sm">
    <thead>
      <tr class="border-border border-b text-left text-muted-foreground">
        {#each columns as col, i (i)}
          <th class="px-4 py-2.5 font-medium {col.class ?? ''}">{col.label}</th>
        {/each}
        <th class="px-4 py-2.5 font-medium text-right">Actions</th>
      </tr>
    </thead>
    <tbody>
      {#each rows as row (getId(row))}
        {@const id = getId(row)}
        {@const isExpanded = expandedId === id}
        {@const highlighted = isHighlighted(id)}
        {@const stale = isStale(row)}
        <tr
          class="border-border border-b cursor-pointer transition-colors duration-700
                 {isExpanded ? 'bg-muted/30' : 'hover:bg-muted/50'}
                 {highlighted ? 'bg-primary/10' : ''}
                 {stale && !highlighted ? 'opacity-60' : ''}"
          onclick={() => onToggle(id)}
        >
          {@render cells(row)}
          <td class="px-4 py-2.5 text-right">
            <div class="inline-flex items-center gap-1">
              {@render actions?.(row)}
              <button
                onclick={(e) => {
                  e.stopPropagation();
                  onToggle(id);
                }}
                class="rounded p-1 text-muted-foreground hover:bg-muted transition-colors cursor-pointer"
                title={isExpanded ? "Collapse" : "Expand details"}
                aria-expanded={isExpanded}
              >
                <ChevronRight
                  size={16}
                  class="transition-transform duration-200 {isExpanded ? 'rotate-90' : ''}"
                />
              </button>
            </div>
          </td>
        </tr>

        {#if isExpanded}
          <tr class="border-border border-b bg-muted/20">
            <td colspan={columns.length + 1} class="p-0 overflow-hidden">
              <div transition:slide={{ duration: 200 }} class="p-4">
                {#if detailLoading}
                  <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
                    <div class="flex flex-col gap-y-3 animate-pulse">
                      <div class="h-4 w-20 rounded bg-muted"></div>
                      <div class="flex flex-col gap-y-2">
                        <div class="h-3 w-36 rounded bg-muted"></div>
                        <div class="h-3 w-44 rounded bg-muted"></div>
                        <div class="h-3 w-28 rounded bg-muted"></div>
                      </div>
                    </div>
                    <div class="lg:col-span-2 flex flex-col gap-y-3 animate-pulse">
                      <div class="h-4 w-32 rounded bg-muted"></div>
                      <div class="flex flex-col gap-y-2">
                        <div class="h-20 rounded bg-muted"></div>
                        <div class="h-20 rounded bg-muted"></div>
                      </div>
                    </div>
                  </div>
                {:else if detailError}
                  <div class="rounded-md bg-destructive/10 px-3 py-2 text-sm text-destructive">
                    {detailError}
                  </div>
                {:else if detail}
                  {@render detailPanel(detail)}
                {/if}
              </div>
            </td>
          </tr>
        {/if}
      {/each}
    </tbody>
  </table>
</div>
