<script module>
import { defineMeta } from "@storybook/addon-svelte-csf";
import ScoreBar from "./ScoreBar.svelte";

const { Story } = defineMeta({
  title: "Components/ScoreBar",
  component: ScoreBar,
  tags: ["autodocs"],
  parameters: {
    layout: "padded",
  },
  argTypes: {
    score: { control: { type: "range", min: 1, max: 5, step: 0.1 } },
    rankScore: { control: { type: "range", min: 1, max: 5, step: 0.1 } },
    ciLower: { control: { type: "range", min: 1, max: 5, step: 0.1 } },
    ciUpper: { control: { type: "range", min: 1, max: 5, step: 0.1 } },
    confidence: { control: { type: "range", min: 0, max: 1, step: 0.01 } },
    source: { control: "select", options: ["both", "rmp", "bluebook"] },
  },
});
</script>

{#snippet barTemplate(args)}
  <div class="w-[480px]"><ScoreBar {...args} /></div>
{/snippet}

<!-- Well rated, both sources agreeing over enough ratings to narrow the interval. -->
<Story
  name="High Score, High Confidence"
  args={{
    score: 4.34,
    rankScore: 4.16,
    ciLower: 4.16,
    ciUpper: 4.51,
    confidence: 0.87,
    source: "both",
    rmpRating: 4.9,
    rmpCount: 21,
    bbRating: 4.74,
    bbCount: 1034,
  }}
  template={barTemplate}
/>

<!-- Middling, and confidently so: a large sample lands the interval tightly. -->
<Story
  name="Medium Score, High Confidence"
  args={{
    score: 3.85,
    rankScore: 3.69,
    ciLower: 3.69,
    ciUpper: 4.0,
    confidence: 0.88,
    source: "both",
    rmpRating: 3.9,
    rmpCount: 86,
    bbRating: 4.44,
    bbCount: 1512,
  }}
  template={barTemplate}
/>

<!-- Poorly rated, with enough ratings that the interval does not rescue it. -->
<Story
  name="Low Score, High Confidence"
  args={{
    score: 2.53,
    rankScore: 2.35,
    ciLower: 2.35,
    ciUpper: 2.71,
    confidence: 0.87,
    source: "both",
    rmpRating: 2.4,
    rmpCount: 73,
    bbRating: 3.54,
    bbCount: 725,
  }}
  template={barTemplate}
/>

<!-- One source only, which widens the interval even with a good sample. -->
<Story
  name="RMP Only"
  args={{
    score: 4.45,
    rankScore: 4.12,
    ciLower: 4.12,
    ciUpper: 4.79,
    confidence: 0.74,
    source: "rmp",
    rmpRating: 4.5,
    rmpCount: 105,
  }}
  template={barTemplate}
/>

<!-- The other single source, to check both render the same way. -->
<Story
  name="BlueBook Only"
  args={{
    score: 4.16,
    rankScore: 3.87,
    ciLower: 3.87,
    ciUpper: 4.44,
    confidence: 0.78,
    source: "bluebook",
    bbRating: 4.68,
    bbCount: 393,
  }}
  template={barTemplate}
/>

<!-- A single rating: the interval spans most of the scale and says so. -->
<Story
  name="Very Low Confidence"
  args={{
    score: 2.16,
    rankScore: 1.31,
    ciLower: 1.31,
    ciUpper: 3.01,
    confidence: 0.35,
    source: "rmp",
    rmpRating: 1.0,
    rmpCount: 1,
  }}
  template={barTemplate}
/>

<!-- Perfect score edge case -->
<Story
  name="Near Perfect"
  args={{
    score: 4.7,
    rankScore: 4.15,
    ciLower: 4.15,
    ciUpper: 5.0,
    confidence: 0.58,
    source: "rmp",
    rmpRating: 5.0,
    rmpCount: 7,
  }}
  template={barTemplate}
/>

<!-- Multiple bars side by side for comparison -->
<Story name="Comparison List">
  {#snippet template()}
    <div class="w-[480px] space-y-2">
      <div class="flex items-center gap-3">
        <span class="w-32 shrink-0 truncate text-sm text-muted-foreground">Adams, Riley</span>
        <ScoreBar score={4.34} rankScore={4.16} ciLower={4.16} ciUpper={4.51} confidence={0.87} source="both" rmpRating={4.9} rmpCount={21} bbRating={4.74} bbCount={1034} class="flex-1" />
      </div>
      <div class="flex items-center gap-3">
        <span class="w-32 shrink-0 truncate text-sm text-muted-foreground">Brooks, Jordan</span>
        <ScoreBar score={3.85} rankScore={3.69} ciLower={3.69} ciUpper={4.0} confidence={0.88} source="both" rmpRating={3.9} rmpCount={86} bbRating={4.44} bbCount={1512} class="flex-1" />
      </div>
      <div class="flex items-center gap-3">
        <span class="w-32 shrink-0 truncate text-sm text-muted-foreground">Chen, Morgan</span>
        <ScoreBar score={4.16} rankScore={3.87} ciLower={3.87} ciUpper={4.44} confidence={0.78} source="bluebook" bbRating={4.68} bbCount={393} class="flex-1" />
      </div>
      <div class="flex items-center gap-3">
        <span class="w-32 shrink-0 truncate text-sm text-muted-foreground">Diaz, Avery</span>
        <ScoreBar score={2.53} rankScore={2.35} ciLower={2.35} ciUpper={2.71} confidence={0.87} source="both" rmpRating={2.4} rmpCount={73} bbRating={3.54} bbCount={725} class="flex-1" />
      </div>
      <div class="flex items-center gap-3">
        <span class="w-32 shrink-0 truncate text-sm text-muted-foreground">Ellis, Sam</span>
        <ScoreBar score={2.16} rankScore={1.31} ciLower={1.31} ciUpper={3.01} confidence={0.35} source="rmp" rmpRating={1.0} rmpCount={1} class="flex-1" />
      </div>
    </div>
  {/snippet}
</Story>
