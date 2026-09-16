<script module>
import { defineMeta } from "@storybook/addon-svelte-csf";
import { expect, userEvent, within } from "storybook/test";
import ScorePopover from "./ScorePopover.svelte";

const { Story } = defineMeta({
  title: "Components/Score/ScorePopover",
  component: ScorePopover,
  tags: ["autodocs"],
  parameters: {
    docs: {
      // The component is a hover target; without autoplay the docs page is a wall of badges.
      story: { autoplay: true, height: "320px" },
    },
  },
});

const bothSources = {
  score: 4.34,
  rankScore: 4.16,
  ciLower: 4.16,
  ciUpper: 4.51,
  confidence: 0.87,
  source: "both",
  totalResponses: 1055,
};

const rmpOnly = {
  score: 4.45,
  rankScore: 4.12,
  ciLower: 4.12,
  ciUpper: 4.79,
  confidence: 0.74,
  source: "rmp",
  totalResponses: 105,
};

const blueBookOnly = {
  score: 4.16,
  rankScore: 3.87,
  ciLower: 3.87,
  ciUpper: 4.44,
  confidence: 0.78,
  source: "bluebook",
  totalResponses: 393,
};

const unrated = {
  score: 3.5,
  rankScore: 1.5,
  ciLower: 1.5,
  ciUpper: 5.0,
  confidence: 0.05,
  source: "rmp",
  totalResponses: 0,
};

/** Hovers the badge and waits for the portalled card to land. */
async function open(canvasElement) {
  const trigger = canvasElement.querySelector("[data-tooltip-trigger]");
  await userEvent.hover(trigger);
  return await within(document.body).findByText("Rating", { selector: "[data-tooltip-content] *" });
}
</script>

{#snippet badge(args)}
  <ScorePopover {...args} />
{/snippet}

<!-- The common case: both sources contribute, and both rows show. -->
<Story
  name="Both Sources"
  args={{
    rating: bothSources,
    rmp: { avgRating: 4.9, numRatings: 21, legacyId: 123456 },
    bluebook: { avgInstructorRating: 4.74, totalResponses: 1034 },
  }}
  template={badge}
  play={async ({ canvasElement }) => {
    await open(canvasElement);
    const card = within(document.body);
    await expect(await card.findByText("BlueBook")).toBeVisible();
    await expect(await card.findByText("RateMyProfessors")).toBeVisible();
  }}
/>

<!-- No BlueBook link, so that row drops out entirely. -->
<Story
  name="RMP Only"
  args={{
    rating: rmpOnly,
    rmp: { avgRating: 4.5, numRatings: 105, legacyId: 234567 },
    bluebook: null,
  }}
  template={badge}
  play={async ({ canvasElement }) => {
    await open(canvasElement);
    await expect(within(document.body).queryByText("BlueBook")).toBeNull();
  }}
/>

<!-- The mirror case: a BlueBook row with no RMP row and no external link. -->
<Story
  name="BlueBook Only"
  args={{
    rating: blueBookOnly,
    rmp: null,
    bluebook: { avgInstructorRating: 4.68, totalResponses: 393 },
  }}
  template={badge}
  play={async ({ canvasElement }) => {
    await open(canvasElement);
    await expect(within(document.body).queryByText("RateMyProfessors")).toBeNull();
  }}
/>

<!-- A matched RMP profile with no ratings on it: no source rows at all, but the link stays. -->
<Story
  name="RMP Linked Without Ratings"
  args={{
    rating: unrated,
    rmp: { avgRating: null, numRatings: null, legacyId: 345678 },
    bluebook: null,
  }}
  template={badge}
  play={async ({ canvasElement }) => {
    await open(canvasElement);
    const card = within(document.body);
    await expect(card.queryByText("RateMyProfessors")).toBeNull();
    await expect(await card.findByText(/View on RateMyProfessors/)).toBeVisible();
  }}
/>

<!-- The same profile alongside BlueBook: one source row, and the RMP row still absent. -->
<Story
  name="RMP Linked Without Ratings, BlueBook Present"
  args={{
    rating: blueBookOnly,
    rmp: { avgRating: null, numRatings: null, legacyId: 345678 },
    bluebook: { avgInstructorRating: 4.68, totalResponses: 393 },
  }}
  template={badge}
  play={async ({ canvasElement }) => {
    await open(canvasElement);
    const card = within(document.body);
    await expect(await card.findByText("BlueBook")).toBeVisible();
    await expect(card.queryByText("RateMyProfessors")).toBeNull();
  }}
/>

<!-- Neither source has anything: the prior shows through, with no source rows. -->
<Story
  name="No Ratings"
  args={{ rating: unrated, rmp: null, bluebook: null }}
  template={badge}
  play={async ({ canvasElement }) => {
    await open(canvasElement);
    await expect(await within(document.body).findByText(/0 total responses/)).toBeVisible();
  }}
/>

<!-- One rating: a confidence interval spanning most of the scale. -->
<Story
  name="Low Confidence"
  args={{
    rating: {
      score: 2.16,
      rankScore: 1.31,
      ciLower: 1.31,
      ciUpper: 3.01,
      confidence: 0.28,
      source: "rmp",
      totalResponses: 1,
    },
    rmp: { avgRating: 1.0, numRatings: 1, legacyId: 456789 },
    bluebook: null,
  }}
  template={badge}
  play={async ({ canvasElement }) => {
    await open(canvasElement);
    await expect(await within(document.body).findByText(/28% confidence/)).toBeVisible();
  }}
/>

<!-- The larger trigger, used where the badge sits on its own rather than in a row. -->
<Story
  name="Small Trigger"
  args={{
    rating: bothSources,
    rmp: { avgRating: 4.9, numRatings: 21, legacyId: 123456 },
    bluebook: { avgInstructorRating: 4.74, totalResponses: 1034 },
    size: "sm",
  }}
  template={badge}
  play={async ({ canvasElement }) => await expect(await open(canvasElement)).toBeVisible()}
/>
