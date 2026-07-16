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
import { expect, fn, mocked, userEvent, within } from "storybook/test";
import { ok } from "true-myth/result";
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

<Story
  name="Default"
  args={{ course: courseWithSeats, expanded: false, onToggle: fn() }}
/>

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

<Story
  name="Interactive"
  args={{ course: courseWithSeats, expanded: false, onToggle: fn() }}
  play={async ({ args, canvasElement }) => {
    const element = canvasElement;
    const canvas = within(element);
    const button = canvas.getByRole("button");

    await expect(button).toBeVisible();
    await expect(button).toHaveAttribute("aria-expanded", "false");
    await userEvent.click(button);
    // @ts-expect-error - args type not fully inferred
    await expect(args.onToggle).toHaveBeenCalled();
  }}
/>
