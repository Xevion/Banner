<script lang="ts">
import { FlexRender } from "$lib/components/ui/data-table/index.js";
import { ArrowDown, ArrowUp, ArrowUpDown } from "@lucide/svelte";
import type { AppTableFeatures } from "$lib/components/ui/data-table/index.js";
import type { HeaderGroup } from "@tanstack/table-core";

/**
 * Replaces a header's asc/desc/none behavior, for a column whose header drives a
 * longer cycle or a different key than its own. Returning null keeps the default.
 */
export interface HeaderOverride {
  /** Replaces the column's own label, with the active sort key or a shared heading. */
  label?: string;
  indicator?: "asc" | "desc" | "none";
  /** Native tooltip, describing what the next click does. */
  title?: string;
  /** Replaces the column's own sort toggle. Omitted, the column keeps it. */
  onclick?: () => void;
}

let {
  headerGroups,
  thClass = "px-3 py-2.5 font-medium whitespace-nowrap",
  sortSpanClass = "inline-flex items-center gap-1",
  checkVisibility = false,
  headerClass,
  headerOverride,
}: {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any -- Generic component accepts any row type
  headerGroups: HeaderGroup<AppTableFeatures, any>[];
  thClass?: string;
  sortSpanClass?: string;
  checkVisibility?: boolean;
  headerClass?: (headerId: string) => string;
  headerOverride?: (headerId: string) => HeaderOverride | null;
} = $props();
</script>

<thead>
  {#each headerGroups as headerGroup (headerGroup.id)}
    <tr class="border-b border-border text-left text-muted-foreground">
      {#each headerGroup.headers as header (header.id)}
        {@const override = headerOverride?.(header.id) ?? null}
        {@const interactive = override?.onclick !== undefined || header.column.getCanSort()}
        {@const sorted = override?.indicator ?? header.column.getIsSorted()}
        {#snippet label()}
          {#if override?.label !== undefined}
            {override.label}
          {:else if typeof header.column.columnDef.header === "string"}
            {header.column.columnDef.header}
          {:else}
            <FlexRender
              content={header.column.columnDef.header}
              context={header.getContext()}
            />
          {/if}
        {/snippet}
        {#if !checkVisibility || header.column.getIsVisible()}
          <th
            class="relative {thClass} {headerClass?.(header.id) ?? ''}"
            title={override?.title}
            aria-sort={interactive
              ? sorted === "asc"
                ? "ascending"
                : sorted === "desc"
                  ? "descending"
                  : "none"
              : undefined}
          >
            {#if interactive}
              <!-- The overlay keeps the whole cell clickable while the focus ring
                   stays on the label, independent of whatever padding thClass sets. -->
              <button
                type="button"
                class="{sortSpanClass} max-w-full cursor-pointer select-none after:absolute after:inset-0 after:content-['']"
                onclick={override?.onclick ?? header.column.getToggleSortingHandler()}
              >
                <!-- Only the text may shrink, so a label wider than its track
                     ellipsizes rather than running into the next column. -->
                <span class="min-w-0 truncate">{@render label()}</span>
                {#if sorted === "asc"}
                  <ArrowUp class="size-3.5 shrink-0" />
                {:else if sorted === "desc"}
                  <ArrowDown class="size-3.5 shrink-0" />
                {:else}
                  <ArrowUpDown class="size-3.5 shrink-0 text-muted-foreground/40" />
                {/if}
              </button>
            {:else}
              <span class="block truncate">{@render label()}</span>
            {/if}
          </th>
        {/if}
      {/each}
    </tr>
  {/each}
</thead>
