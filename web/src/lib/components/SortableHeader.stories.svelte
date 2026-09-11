<script module>
import CourseHeaderHarness from "$lib/stories/CourseHeaderHarness.svelte";
import { mockCourses } from "$lib/stories/fixtures/courses";
import NarrowHeaderHarness from "$lib/stories/NarrowHeaderHarness.svelte";
import { defineMeta } from "@storybook/addon-svelte-csf";
import { expect, within } from "storybook/test";

const { Story } = defineMeta({
  title: "Components/CourseTable/SortableHeader",
  component: CourseHeaderHarness,
  tags: ["autodocs"],
  parameters: { layout: "fullscreen" },
  args: { courses: mockCourses, subjectMap: { CS: "Computer Science", MAT: "Mathematics" } },
});

const withEndTime = { time_end: true };

const headerNames = (canvasElement) =>
  within(canvasElement)
    .getAllByRole("columnheader")
    .map((th) => th.textContent?.trim() ?? "");

/** No header may be wider than the track it sits in, whatever it has to say. */
const expectNoOverflow = async (canvasElement) => {
  for (const th of within(canvasElement).getAllByRole("columnheader")) {
    const button = within(th).queryByRole("button");
    if (!button) continue;
    await expect(button.getBoundingClientRect().width).toBeLessThanOrEqual(
      th.getBoundingClientRect().width
    );
  }
};
</script>

<Story
  name="Unsorted"
  args={{ sort: "" }}
  play={async ({ canvasElement }) => {
    await expect(headerNames(canvasElement)).toContain("Duration");
    await expect(headerNames(canvasElement)).toContain("Instructor");
    await expectNoOverflow(canvasElement);
  }}
/>

<!-- A column offering one key keeps its own name whichever way it points. -->
<Story
  name="Start Time"
  args={{ sort: "start_time" }}
  play={async ({ canvasElement }) => {
    await expect(headerNames(canvasElement)).toContain("Start Time");
    await expectNoOverflow(canvasElement);
  }}
/>

<!-- Duration is the narrowest sortable track at 84px, so it is the column a key
     name has to fit inside. Its second key is named, not appended. -->
<Story
  name="Duration"
  args={{ sort: "duration" }}
  play={async ({ canvasElement }) => {
    await expect(headerNames(canvasElement)).toContain("Duration");
    await expectNoOverflow(canvasElement);
  }}
/>

<Story
  name="Weekly Minutes"
  args={{ sort: "-weekly_minutes" }}
  play={async ({ canvasElement }) => {
    await expect(headerNames(canvasElement)).toContain("Mins/Wk");
    await expect(headerNames(canvasElement)).not.toContain("Duration");
    await expectNoOverflow(canvasElement);
  }}
/>

<Story
  name="Instructor Rating"
  args={{ sort: "-instructor_rating" }}
  play={async ({ canvasElement }) => {
    await expect(headerNames(canvasElement)).toContain("Instructor Rating");
    await expectNoOverflow(canvasElement);
  }}
/>

<Story
  name="Instructor Name"
  args={{ sort: "instructor_name" }}
  play={async ({ canvasElement }) => {
    await expect(headerNames(canvasElement)).toContain("Instructor Name");
    await expectNoOverflow(canvasElement);
  }}
/>

<Story
  name="Open Seats"
  args={{ sort: "-seats_open" }}
  play={async ({ canvasElement }) => {
    await expect(headerNames(canvasElement)).toContain("Open Seats");
    await expectNoOverflow(canvasElement);
  }}
/>

<Story
  name="Fill Ratio"
  args={{ sort: "fill_ratio" }}
  play={async ({ canvasElement }) => {
    await expect(headerNames(canvasElement)).toContain("Fill Ratio");
    await expectNoOverflow(canvasElement);
  }}
/>

<!-- Both halves of the range shown, which merges the pair under one label and
     leaves the end column's own heading to screen readers. -->
<Story
  name="Time Pair"
  args={{ sort: "start_time", columnVisibility: withEndTime }}
  play={async ({ canvasElement }) => {
    await expect(headerNames(canvasElement)).toContain("Time");
    await expect(within(canvasElement).getByRole("button", { name: "End Time" })).toBeVisible();
    await expectNoOverflow(canvasElement);
  }}
/>

<Story
  name="Time Pair Sorted By End"
  args={{ sort: "-end_time", columnVisibility: withEndTime }}
  play={async ({ canvasElement }) => await expectNoOverflow(canvasElement)}
/>

<!-- The guard the whole table leans on: a label wider than its track ellipsizes
     inside the cell, and the arrow carrying the sort state survives it. -->
<Story
  name="Overflowing Labels"
  asChild
  play={async ({ canvasElement }) => {
    const [narrow] = within(canvasElement).getAllByRole("columnheader");
    const label = within(narrow).getByText("Weekly Minutes");
    await expect(label.scrollWidth).toBeGreaterThan(label.clientWidth);
    await expectNoOverflow(canvasElement);
  }}
>
  <NarrowHeaderHarness
    columns={[
      { id: "narrow", label: "Weekly Minutes", width: 84 },
      { id: "tiny", label: "Instructor Rating", width: 60 },
      { id: "roomy", label: "Instructor Rating", width: 200 },
    ]}
  />
</Story>
