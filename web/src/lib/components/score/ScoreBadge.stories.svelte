<script module>
import { defineMeta } from "@storybook/addon-svelte-csf";
import ScoreBadge from "./ScoreBadge.svelte";

const { Story } = defineMeta({
  title: "Components/Score/ScoreBadge",
  component: ScoreBadge,
  tags: ["autodocs"],
  argTypes: {
    score: { control: { type: "range", min: 1, max: 5, step: 0.1 } },
    confidence: { control: { type: "range", min: 0, max: 1, step: 0.01 } },
    size: { control: "select", options: ["xs", "sm", "lg"] },
  },
});

const TIERS = [
  { confidence: 0.87, label: "High (solid border, star)" },
  { confidence: 0.4, label: "Medium (dashed border, star)" },
  { confidence: 0.18, label: "Low (dashed border, triangle)" },
];

const SIZES = ["xs", "sm", "lg"];
</script>

<!-- Enough ratings to be trusted: solid, starred, and green. -->
<Story name="High Confidence" args={{ score: 4.3, confidence: 0.87, size: "sm" }} />

<!-- Sparse but usable, so the border goes dashed while the star stays. -->
<Story name="Medium Confidence" args={{ score: 3.5, confidence: 0.4, size: "sm" }} />

<!-- A single response: the triangle replaces the star as a warning. -->
<Story name="Low Confidence" args={{ score: 2.1, confidence: 0.18, size: "sm" }} />

<!-- The colour ramp across the scale, at one confidence, to check the gradient. -->
<Story name="Score Range">
  {#snippet template()}
    <div class="flex items-center gap-2">
      {#each [1.0, 1.8, 2.5, 3.2, 3.8, 4.4, 5.0] as score (score)}
        <ScoreBadge {score} confidence={0.87} size="sm" />
      {/each}
    </div>
  {/snippet}
</Story>

<!-- The three sizes the app actually asks for, side by side. -->
<Story name="Sizes">
  {#snippet template()}
    <div class="flex items-center gap-3">
      {#each SIZES as size (size)}
        <ScoreBadge score={4.3} confidence={0.87} {size} />
      {/each}
    </div>
  {/snippet}
</Story>

<!-- Every confidence tier at every size: the whole visual surface in one shot. -->
<Story name="Confidence Tiers">
  {#snippet template()}
    <div class="flex flex-col gap-3">
      {#each TIERS as tier (tier.label)}
        <div class="flex items-center gap-3">
          <span class="w-56 text-xs text-muted-foreground">{tier.label}</span>
          {#each SIZES as size (size)}
            <ScoreBadge score={3.9} confidence={tier.confidence} {size} />
          {/each}
        </div>
      {/each}
    </div>
  {/snippet}
</Story>
