<script lang="ts">
import { FlexRender } from "$lib/components/ui/data-table/index.js";
import { ArrowDown, ArrowUp, ArrowUpDown } from "@lucide/svelte";
import type { HeaderGroup } from "@tanstack/table-core";

let {
  headerGroups,
  thClass = "px-3 py-2.5 font-medium whitespace-nowrap",
  sortSpanClass = "inline-flex items-center gap-1",
  checkVisibility = false,
  headerClass,
}: {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any -- Generic component accepts any row type
  headerGroups: HeaderGroup<any>[];
  thClass?: string;
  sortSpanClass?: string;
  checkVisibility?: boolean;
  headerClass?: (headerId: string) => string;
} = $props();
</script>

<thead>
  {#each headerGroups as headerGroup (headerGroup.id)}
    <tr class="border-b border-border text-left text-muted-foreground">
      {#each headerGroup.headers as header (header.id)}
        {#if !checkVisibility || header.column.getIsVisible()}
          <th
            class="{thClass} {headerClass?.(header.id) ?? ''}"
            class:cursor-pointer={header.column.getCanSort()}
            class:select-none={header.column.getCanSort()}
            onclick={header.column.getToggleSortingHandler()}
          >
            {#if header.column.getCanSort()}
              <span class={sortSpanClass}>
                {#if typeof header.column.columnDef.header === "string"}
                  {header.column.columnDef.header}
                {:else}
                  <FlexRender
                    content={header.column.columnDef.header}
                    context={header.getContext()}
                  />
                {/if}
                {#if header.column.getIsSorted() === "asc"}
                  <ArrowUp class="size-3.5" />
                {:else if header.column.getIsSorted() === "desc"}
                  <ArrowDown class="size-3.5" />
                {:else}
                  <ArrowUpDown class="size-3.5 text-muted-foreground/40" />
                {/if}
              </span>
            {:else if typeof header.column.columnDef.header === "string"}
              {header.column.columnDef.header}
            {:else}
              <FlexRender
                content={header.column.columnDef.header}
                context={header.getContext()}
              />
            {/if}
          </th>
        {/if}
      {/each}
    </tr>
  {/each}
</thead>
