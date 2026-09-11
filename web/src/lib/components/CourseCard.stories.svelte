<script module>
import { client } from "$lib/api";
import CourseDetailDecorator from "$lib/stories/CourseDetailDecorator.svelte";
import {
  courseWithSeats,
  fullCourse,
  lowSeatsCourse,
  onlineCourse,
  relatedSections,
  staffInstructorCourse,
} from "$lib/stories/fixtures/courses";
import { defineMeta } from "@storybook/addon-svelte-csf";
import { expect, fn, mocked, userEvent, waitFor, within } from "storybook/test";
import { ok } from "true-myth/result";
import StatefulCourseCard from "$lib/stories/StatefulCourseCard.svelte";
import CourseCard from "./CourseCard.svelte";

const { Story } = defineMeta({
  title: "Components/CourseCard",
  component: CourseCard,
  tags: ["autodocs"],
  parameters: {
    layout: "padded",
  },
  beforeEach: () => {
    mocked(client.getRelatedSections).mockResolvedValue(ok(relatedSections));
  },
  decorators: [
    (storyFn) => {
      storyFn();
      return { Component: CourseDetailDecorator };
    },
  ],
});
</script>

<!--
  Expanding here really works, because the header invites a click and a card
  that ignores one is the first thing anyone tries. The stories below pin a
  state on purpose and do not respond, which is what they are for.
-->
<Story name="Default" args={{ course: courseWithSeats }}>
  {#snippet template(args)}
    <StatefulCourseCard {...args} />
  {/snippet}
</Story>

<Story
  name="Expanded"
  args={{ course: courseWithSeats, expanded: true, onToggle: fn() }}
  play={async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const sibling = await canvas.findByText("12352");

    await expect(sibling).toBeVisible();
    await expect(canvas.queryByText(/Network request failed/i)).not.toBeInTheDocument();
  }}
/>

<Story
  name="Full Class"
  args={{ course: fullCourse, expanded: false, onToggle: fn() }}
/>

<Story
  name="Online Course"
  args={{ course: onlineCourse, expanded: false, onToggle: fn() }}
/>

<Story
  name="Low Seats"
  args={{ course: lowSeatsCourse, expanded: false, onToggle: fn() }}
/>

<Story
  name="Staff Instructor"
  args={{ course: staffInstructorCourse, expanded: false, onToggle: fn() }}
/>

<!-- Clicking the header has to actually open the card, not just fire a handler. -->
<Story
  name="Interactive"
  args={{ course: courseWithSeats }}
  play={async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const header = canvas.getAllByRole("button")[0];

    await expect(header).toHaveAttribute("aria-expanded", "false");
    await userEvent.click(header);
    await waitFor(async () => {
      await expect(header).toHaveAttribute("aria-expanded", "true");
    });
    // The detail slides open, so it is in the DOM a moment before it is on screen.
    const heading = await canvas.findByText("Other Sections");
    await waitFor(async () => {
      await expect(heading).toBeVisible();
    });
  }}
>
  {#snippet template(args)}
    <StatefulCourseCard {...args} />
  {/snippet}
</Story>
