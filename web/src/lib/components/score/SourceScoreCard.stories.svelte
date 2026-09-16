<script module>
import { defineMeta } from "@storybook/addon-svelte-csf";
import SourceScoreCard from "./SourceScoreCard.svelte";

const { Story } = defineMeta({
  title: "Components/Score/SourceScoreCard",
  component: SourceScoreCard,
  tags: ["autodocs"],
  parameters: {
    layout: "padded",
  },
  argTypes: {
    source: { control: "select", options: ["bluebook", "rmp"] },
    inline: { control: "boolean" },
  },
});

const blueBook = {
  calibratedRating: 4.16,
  avgInstructorRating: 4.68,
  avgCourseRating: 4.41,
  totalResponses: 1034,
  evalCount: 27,
};

const rmp = {
  avgRating: 4.5,
  avgDifficulty: 2.8,
  wouldTakeAgainPct: 87.4,
  numRatings: 105,
  legacyId: 123456,
};
</script>

{#snippet card(args)}
  <div class="w-[520px]"><SourceScoreCard {...args} /></div>
{/snippet}

<!-- A BlueBook record with every stat present. -->
<Story name="BlueBook" args={{ source: "bluebook", bluebook: blueBook }} template={card} />

<!-- Courses were never rated separately, so that stat drops out. -->
<Story
  name="BlueBook Without Course Rating"
  args={{ source: "bluebook", bluebook: { ...blueBook, avgCourseRating: null } }}
  template={card}
/>

<!-- A single evaluation: the numbers stand, thin as they are. -->
<Story
  name="BlueBook Single Response"
  args={{
    source: "bluebook",
    bluebook: {
      calibratedRating: 2.4,
      avgInstructorRating: 2.0,
      avgCourseRating: null,
      totalResponses: 1,
      evalCount: 1,
    },
  }}
  template={card}
/>

<!-- An RMP profile with difficulty and would-take-again filled in. -->
<Story name="RMP" args={{ source: "rmp", rmp }} template={card} />

<!-- A matched profile that nobody has rated: the whole stat row collapses. -->
<Story
  name="RMP Without Ratings"
  args={{
    source: "rmp",
    rmp: { avgRating: null, avgDifficulty: null, wouldTakeAgainPct: null, numRatings: null, legacyId: 234567 },
  }}
  template={card}
/>

<!-- RMP returns a rating but omits the optional fields often enough to matter. -->
<Story
  name="RMP Partial Stats"
  args={{
    source: "rmp",
    rmp: { avgRating: 3.2, avgDifficulty: null, wouldTakeAgainPct: null, numRatings: 4, legacyId: 345678 },
  }}
  template={card}
/>

<!-- Inline drops the card chrome and header for hosts that supply their own. -->
<Story
  name="Inline"
  args={{ source: "bluebook", bluebook: blueBook, inline: true }}
  template={card}
/>

<!-- Both sources stacked, the way an instructor profile shows them. -->
<Story name="Both Sources">
  {#snippet template()}
    <div class="w-[520px] flex flex-col gap-3">
      <SourceScoreCard source="bluebook" bluebook={blueBook} />
      <SourceScoreCard source="rmp" {rmp} />
    </div>
  {/snippet}
</Story>
