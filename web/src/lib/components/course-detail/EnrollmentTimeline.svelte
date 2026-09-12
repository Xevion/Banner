<script lang="ts">
import { type HistoryPoint, historyScaleMax, toDayRows } from "$lib/enrollment-history";
import { formatNumber } from "$lib/utils";

type Spacing = "change" | "day";

let {
  points,
  spacing = $bindable<Spacing>("change"),
  pageSize = 40,
}: { points: HistoryPoint[]; spacing?: Spacing; pageSize?: number } = $props();

type Row = HistoryPoint & { held?: boolean };

let rows = $derived<Row[]>(spacing === "day" ? toDayRows(points) : [...points].reverse());

// Bars share one absolute scale rather than normalising to full width, so a capacity
// change reads as the bar growing instead of being flattened away.
let scaleMax = $derived(historyScaleMax(points));

let shown = $state(0);

// Reset paging whenever the underlying rows change, or a spacing switch would keep
// scrolling from wherever the previous list left off.
$effect(() => {
  void rows;
  shown = pageSize;
});

let visible = $derived(rows.slice(0, shown));
let exhausted = $derived(shown >= rows.length);

function loadMore() {
  if (!exhausted) shown += pageSize;
}

function onScroll(event: Event) {
  const el = event.currentTarget as HTMLElement;
  if (el.scrollTop + el.clientHeight >= el.scrollHeight - 120) loadMore();
}

function pct(value: number): number {
  return Math.max(0, Math.min(100, (value / scaleMax) * 100));
}

function label(row: Row): string {
  if (spacing === "day") {
    return row.date.toLocaleDateString("en-US", {
      month: "short",
      day: "numeric",
      year: "2-digit",
    });
  }
  return (
    row.date.toLocaleDateString("en-US", { month: "short", day: "numeric" }) +
    ", " +
    row.date.toLocaleTimeString("en-US", { hour: "numeric", minute: "2-digit" })
  );
}
</script>

<div class="flex flex-col gap-2">
  <div
    class="flex items-center justify-between gap-3 flex-wrap border-b border-border pb-2 font-mono text-[10px] tracking-wider text-muted-foreground uppercase"
  >
    <span>Newest first</span>
    <span class="flex gap-3 normal-case tracking-normal">
      <span class="inline-flex items-center gap-1.5">
        <span class="inline-block size-2 rounded-[2px] bg-status-blue"></span>Taken
      </span>
      <span class="inline-flex items-center gap-1.5">
        <span class="inline-block size-2 rounded-[2px] border border-border bg-muted"></span>Open
      </span>
      <span class="inline-flex items-center gap-1.5">
        <span class="inline-block size-2 rounded-[2px] bg-status-orange"></span>Waiting
      </span>
    </span>
  </div>

  <div
    class="max-h-[360px] overflow-y-auto overflow-x-hidden overscroll-contain pr-1"
    onscroll={onScroll}
  >
    {#each visible as row (row.date.getTime())}
      {@const capPct = pct(row.capacity)}
      <div
        class="grid grid-cols-[6rem_minmax(0,1fr)_5rem] items-center gap-2 h-5 text-[11px] {row.held
          ? 'opacity-45'
          : ''}"
      >
        <span class="truncate font-mono text-[10px] text-muted-foreground">{label(row)}</span>

        <span
          class="relative h-[13px] rounded-[2px] border border-border bg-muted overflow-hidden"
          data-tooltip="{formatNumber(row.enrolled)}/{formatNumber(row.capacity)} enrolled, {formatNumber(
            row.open
          )} open{row.waiting > 0 ? `, ${formatNumber(row.waiting)} waiting` : ''}"
          data-tooltip-delay="200"
        >
          <span class="absolute inset-y-0 left-0 bg-status-blue" style="width: {pct(row.enrolled)}%"
          ></span>
          {#if row.waiting > 0}
            <span
              class="absolute inset-y-0 bg-status-orange"
              style="left: {capPct}%; width: {Math.max(0.6, Math.min(100 - capPct, pct(row.waiting)))}%"
            ></span>
          {/if}
          <span class="absolute -inset-y-px w-px bg-muted-foreground" style="left: {capPct}%"></span>
        </span>

        <span class="text-right font-mono text-[10px] tabular-nums text-muted-foreground">
          {formatNumber(row.open)} open{row.waiting > 0 ? ` +${formatNumber(row.waiting)}` : ""}
        </span>
      </div>
    {/each}

    <p
      class="py-2 text-center font-mono text-[10px] tracking-wider uppercase {exhausted
        ? 'text-border'
        : 'text-muted-foreground'}"
    >
      {#if exhausted}
        Start of recorded history &middot; {formatNumber(rows.length)}
        {spacing === "day" ? "days" : "changes"}
      {:else}
        Scroll for earlier snapshots
      {/if}
    </p>
  </div>
</div>
