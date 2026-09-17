<script module>
import { client } from "$lib/api";
import CourseDetailDecorator from "$lib/stories/CourseDetailDecorator.svelte";
import { mockCourses, relatedSections } from "$lib/stories/fixtures/courses";
import { defineMeta } from "@storybook/addon-svelte-csf";
import { expect, mocked, waitFor, within } from "storybook/test";
import { ok } from "true-myth/result";
import CourseTable from "./CourseTable.svelte";

const { Story } = defineMeta({
  title: "Components/CourseTable",
  component: CourseTable,
  parameters: {
    layout: "padded",
  },
  beforeEach: () => {
    mocked(client.getRelatedSections).mockResolvedValue(ok(relatedSections));
    mocked(client.getCourseTrends).mockResolvedValue(ok({ trends: {} }));
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
  The card and table layouts are both in the DOM at every width, with CSS
  choosing between them. Opening a row must still load its detail once: a
  panel behind `display: none` fetches exactly as eagerly as a visible one.
-->
<Story
  name="Expands Once"
  args={{ courses: mockCourses, loading: false }}
  play={async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    mocked(client.getRelatedSections).mockClear();

    // The row is clicked directly rather than through userEvent because only one
    // of the two layouts is on screen at the runner's width, and which one that
    // is has no bearing on how many times the panel behind it loads.
    const table = canvasElement.querySelector("table");
    const row = table.querySelector("tbody tr");
    await expect(row).not.toBeNull();
    row.click();

    await canvas.findAllByText("Other Sections");
    await waitFor(async () => {
      const panels = canvas.getAllByText("Other Sections");
      await expect(panels).toHaveLength(1);
      // The survivor has to be the layout on screen, not the one CSS hid. Which
      // one that is follows the `sm:` breakpoint, whatever width the runner uses.
      await expect(panels[0]).toBeVisible();
      await expect(table.contains(panels[0])).toBe(window.innerWidth >= 640);
      await expect(client.getRelatedSections).toHaveBeenCalledTimes(1);
    });
  }}
/>
