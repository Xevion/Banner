<script module>
import { defineMeta } from "@storybook/addon-svelte-csf";
import InstructorCard from "./InstructorCard.svelte";

const { Story } = defineMeta({
  title: "Components/InstructorCard",
  component: InstructorCard,
  tags: ["autodocs"],
  parameters: {
    layout: "padded",
  },
  argTypes: {
    variant: { control: "select", options: ["grid", "panel", "compact"] },
  },
});

const rating = {
  score: 4.34,
  rankScore: 4.16,
  ciLower: 4.16,
  ciUpper: 4.51,
  confidence: 0.87,
  source: "both",
  totalResponses: 1055,
};

const sparseRating = {
  score: 2.16,
  rankScore: 1.31,
  ciLower: 1.31,
  ciUpper: 3.01,
  confidence: 0.28,
  source: "rmp",
  totalResponses: 1,
};

const rmp = { avgRating: 4.9, numRatings: 21, legacyId: 123456 };
const bluebook = { avgInstructorRating: 4.74, totalResponses: 1034 };

const SUBJECT_NAMES = {
  CS: "Computer Science",
  MAT: "Mathematics",
  IS: "Information Systems",
  STA: "Statistics",
  PHY: "Physics",
  EGR: "Engineering",
};

const subjectLabel = (code) => SUBJECT_NAMES[code] ?? code;
</script>

{#snippet gridTemplate(args)}
  <div class="w-[340px]"><InstructorCard {...args} /></div>
{/snippet}

{#snippet rowTemplate(args)}
  <div class="w-[420px]"><InstructorCard {...args} /></div>
{/snippet}

<!-- The directory card: whole-card link, email, and subject chips that link away. -->
<Story
  name="Grid"
  args={{
    variant: "grid",
    name: "Adams, Riley",
    slug: "adams-riley-4kp",
    email: "riley.adams@utsa.edu",
    subjects: ["CS", "MAT"],
    subjectLabel,
    rating,
  }}
  template={gridTemplate}
/>

<!-- More subjects than fit, so the overflow count takes over. -->
<Story
  name="Grid With Subject Overflow"
  args={{
    variant: "grid",
    name: "Brooks, Jordan",
    slug: "brooks-jordan-9tv",
    email: "jordan.brooks@my.utsa.edu",
    subjects: ["CS", "MAT", "IS", "STA", "PHY", "EGR"],
    subjectLabel,
    rating,
  }}
  template={gridTemplate}
/>

<!-- No rating and no email: the card keeps its shape on the sparsest record. -->
<Story
  name="Grid Without Rating"
  args={{
    variant: "grid",
    name: "Castellanos-Villanueva, Maximiliano Alejandro",
    slug: "castellanos-villanueva-maximiliano-2qd",
    subjects: ["EGR"],
    subjectLabel,
  }}
  template={gridTemplate}
/>

<!-- The course-level panel: one row per instructor across every section. -->
<Story
  name="Panel"
  args={{
    variant: "panel",
    name: "Riley Adams",
    slug: "adams-riley-4kp",
    rating,
    rmp,
    bluebook,
  }}
  template={rowTemplate}
/>

<!-- The course-detail row, tighter, with the primary-instructor badge. -->
<Story
  name="Compact"
  args={{
    variant: "compact",
    name: "Riley Adams",
    slug: "adams-riley-4kp",
    badge: "Primary",
    rating,
    rmp,
    bluebook,
  }}
  template={rowTemplate}
/>

<!-- A single rating: the badge goes dashed and swaps its icon to warn. -->
<Story
  name="Compact Low Confidence"
  args={{
    variant: "compact",
    name: "Sam Ellis",
    slug: "ellis-sam-7wb",
    rating: sparseRating,
    rmp: { avgRating: 1.0, numRatings: 1, legacyId: 987654 },
  }}
  template={rowTemplate}
/>

<!-- Banner names an instructor we have no record for: plain text, no link. -->
<Story
  name="Unlinked"
  args={{
    variant: "panel",
    name: "Morgan Chen",
    slug: null,
  }}
  template={rowTemplate}
/>

<!-- A name long enough to truncate, in the narrowest row the app renders. -->
<Story
  name="Long Name Truncation"
  args={{
    variant: "compact",
    name: "Maximiliano Alejandro Castellanos-Villanueva",
    slug: "castellanos-villanueva-maximiliano-2qd",
    badge: "Primary",
    rating,
    rmp,
    bluebook,
  }}
  template={rowTemplate}
/>

<!-- Every variant together, to check they read as the same concept. -->
<Story name="All Variants">
  {#snippet template()}
    <div class="flex flex-col gap-4 w-[340px]">
      <InstructorCard
        variant="grid"
        name="Adams, Riley"
        slug="adams-riley-4kp"
        email="riley.adams@utsa.edu"
        subjects={["CS", "MAT"]}
        {subjectLabel}
        {rating}
      />
      <InstructorCard
        variant="panel"
        name="Riley Adams"
        slug="adams-riley-4kp"
        {rating}
        {rmp}
        {bluebook}
      />
      <InstructorCard
        variant="compact"
        name="Riley Adams"
        slug="adams-riley-4kp"
        badge="Primary"
        {rating}
        {rmp}
        {bluebook}
      />
    </div>
  {/snippet}
</Story>
