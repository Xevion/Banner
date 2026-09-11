<script lang="ts">
import SortableHeader from "$lib/components/SortableHeader.svelte";
import { APP_TABLE_FEATURES, createSvelteTable } from "$lib/components/ui/data-table/index.js";

type Row = Record<string, string>;

let {
  columns,
}: {
  /** Fixed track widths and the labels to squeeze into them. */
  columns: { id: string; label: string; width: number }[];
} = $props();

// Named outright: a fixed layout still sizes an auto-width table by its content
// first, which would hand a track more room than the colgroup asked for.
const totalWidth = $derived(columns.reduce((total, column) => total + column.width, 0));

const table = createSvelteTable({
  features: APP_TABLE_FEATURES,
  data: [] as Row[],
  get columns() {
    return columns.map((column) => ({
      id: column.id,
      header: column.label,
      accessorFn: () => "",
    }));
  },
});
</script>

<table class="table-fixed border-collapse text-sm" style:width="{totalWidth}px">
  <colgroup>
    {#each columns as column (column.id)}
      <col style:width="{column.width}px" />
    {/each}
  </colgroup>
  <SortableHeader
    headerGroups={table.getHeaderGroups()}
    thClass="px-2 pb-1.5 text-[10px] font-semibold tracking-[0.09em] uppercase text-muted-foreground select-none"
  />
  <tbody>
    <tr class="h-10 border-b border-border/60">
      {#each columns as column (column.id)}
        <td class="truncate px-2 text-muted-foreground/50">{column.width}px</td>
      {/each}
    </tr>
  </tbody>
</table>
