<script lang="ts">
import type { ColumnDef } from "@tanstack/table-core";

let {
  columns,
  rowCount = 5,
  skeletonWidths = {},
  cellClass = "px-3 py-2.5",
  rowHeight = "h-3.5",
  defaultWidth = "w-20",
}: {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any -- Generic component accepts any column type
  columns: ColumnDef<any, any>[];
  rowCount?: number;
  skeletonWidths?: Record<string, string>;
  cellClass?: string;
  rowHeight?: string;
  defaultWidth?: string;
} = $props();
</script>

<tbody>
  {#each Array(rowCount) as _, i (i)}
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
