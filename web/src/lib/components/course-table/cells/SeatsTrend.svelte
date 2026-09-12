<script lang="ts">
import type { TrendSample } from "$lib/bindings";

let {
  samples,
  width = 32,
  height = 12,
}: { samples: TrendSample[]; width?: number; height?: number } = $props();

// Same encoding as the History tab's chart: blue enrolled, orange waiting, one shared
// scale. The point is movement -- a full section that churns reads differently from a
// full section that has not moved since April.
const PAD = 1.5;

let scale = $derived(
  Math.max(
    1,
    ...samples.map((s) => s.enrollment + s.seatsAvailable),
    ...samples.map((s) => s.waitCount)
  )
);

function path(values: number[]): string {
  if (values.length < 2) return "";
  const span = values.length - 1;
  return values
    .map((value, i) => {
      const x = PAD + (i / span) * (width - PAD * 2);
      const y = height - PAD - Math.min(1, value / scale) * (height - PAD * 2);
      const prev =
        i === 0 ? y : height - PAD - Math.min(1, values[i - 1] / scale) * (height - PAD * 2);
      return i === 0
        ? `M${x.toFixed(1)} ${y.toFixed(1)}`
        : `L${x.toFixed(1)} ${prev.toFixed(1)}L${x.toFixed(1)} ${y.toFixed(1)}`;
    })
    .join("");
}

let enrolledPath = $derived(path(samples.map((s) => s.enrollment)));
let waitingPath = $derived(
  samples.some((s) => s.waitCount > 0) ? path(samples.map((s) => s.waitCount)) : ""
);
</script>

{#if enrolledPath}
  <svg
    viewBox="0 0 {width} {height}"
    {width}
    {height}
    class="block"
    aria-hidden="true"
  >
    {#if waitingPath}
      <path
        d={waitingPath}
        fill="none"
        stroke="var(--status-orange)"
        stroke-width="1"
        stroke-linejoin="round"
      />
    {/if}
    <path
      d={enrolledPath}
      fill="none"
      stroke="var(--status-blue)"
      stroke-width="1.25"
      stroke-linejoin="round"
    />
  </svg>
{/if}
