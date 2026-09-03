<script lang="ts">
let {
  statCards = 5,
  statCardsClass = "grid-cols-2 sm:grid-cols-3 lg:grid-cols-5",
  rows = 8,
  cellClasses,
}: {
  statCards?: number;
  statCardsClass?: string;
  rows?: number;
  /** One entry per column after the name/subtitle block, e.g. "h-5 w-16 rounded-full". */
  cellClasses: string[];
} = $props();
</script>

<div class="mb-4 grid {statCardsClass} gap-3">
  {#each Array(statCards) as _stat, i (i)}
    <div class="bg-card border-border rounded-lg border p-3">
      <div class="h-3 w-16 animate-pulse rounded bg-muted mb-2"></div>
      <div class="h-6 w-12 animate-pulse rounded bg-muted"></div>
    </div>
  {/each}
</div>
<div class="bg-muted mb-6 h-2 rounded-full overflow-hidden"></div>
<div class="bg-card border-border overflow-hidden rounded-lg border">
  <div>
    {#each Array(rows) as _row, i (i)}
      <div class="flex items-center gap-4 px-4 py-3{i > 0 ? ' border-t border-border' : ''}">
        <div class="flex flex-col gap-y-1.5 flex-1">
          <div class="h-4 w-40 animate-pulse rounded bg-muted"></div>
          <div class="h-3 w-28 animate-pulse rounded bg-muted"></div>
        </div>
        {#each cellClasses as cell, c (c)}
          <div class="animate-pulse bg-muted {cell}"></div>
        {/each}
      </div>
    {/each}
  </div>
</div>
