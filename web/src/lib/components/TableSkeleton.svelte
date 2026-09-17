<script lang="ts" generics="TData extends RowData">
import { range } from "$lib/utils";
import type { AppTableFeatures } from "$lib/components/ui/data-table/index.js";
import type { ColumnDef, RowData } from "@tanstack/table-core";

let {
  columns,
  rowCount = 5,
  skeletonWidths = {},
  cellClass = "px-3 py-2.5",
  rowHeight = "h-3.5",
  defaultWidth = "w-20",
}: {
  columns: ColumnDef<AppTableFeatures, TData>[];
  rowCount?: number;
  skeletonWidths?: Record<string, string>;
  cellClass?: string;
  rowHeight?: string;
  defaultWidth?: string;
} = $props();
</script>

<tbody>
  {#each range(rowCount) as i (i)}
    <tr class="border-b border-border">
      {#each columns as col (col.id)}
        <td class={cellClass}>
          <div
            class="{rowHeight} rounded bg-muted animate-pulse {skeletonWidths[col.id ?? ''] ??
              defaultWidth}"
          ></div>
        </td>
      {/each}
    </tr>
  {/each}
</tbody>
