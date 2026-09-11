<script lang="ts">
import { formatCompactTime, parseTimeInput } from "$lib/filters";

let {
  timeStart = $bindable<string | null>(null),
  timeEnd = $bindable<string | null>(null),
}: {
  timeStart: string | null;
  timeEnd: string | null;
} = $props();
</script>

<div class="flex flex-col gap-1.5" role="group" aria-label="Time range">
  <span class="text-xs font-medium text-muted-foreground select-none" aria-hidden="true">
    Time range
  </span>
  <div class="flex items-center gap-2">
    <input
      type="text"
      placeholder="10:00 AM"
      aria-label="Earliest start time"
      autocomplete="off"
      value={formatCompactTime(timeStart)}
      onchange={(e) => {
        timeStart = parseTimeInput(e.currentTarget.value);
        e.currentTarget.value = formatCompactTime(timeStart);
      }}
      class="h-8 w-24 border border-border bg-card text-foreground rounded-md px-2 text-sm
             focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background"
    />
    <span class="text-xs text-muted-foreground select-none">to</span>
    <input
      type="text"
      placeholder="3:00 PM"
      aria-label="Latest end time"
      autocomplete="off"
      value={formatCompactTime(timeEnd)}
      onchange={(e) => {
        timeEnd = parseTimeInput(e.currentTarget.value);
        e.currentTarget.value = formatCompactTime(timeEnd);
      }}
      class="h-8 w-24 border border-border bg-card text-foreground rounded-md px-2 text-sm
             focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background"
    />
  </div>
</div>
