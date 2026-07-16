<script module>
import {
  courseWithSeats,
  fullCourse,
  lowSeatsCourse,
  overenrolledCourse,
} from "$lib/stories/fixtures/courses";
import TableRowDecorator from "$lib/stories/TableRowDecorator.svelte";
import { defineMeta } from "@storybook/addon-svelte-csf";
import SeatsCell from "./SeatsCell.svelte";

const { Story } = defineMeta({
  title: "Components/CourseTable/Cells/SeatsCell",
  component: SeatsCell,
  tags: ["autodocs"],
  decorators: [
    (storyFn) => {
      storyFn();
      return { Component: TableRowDecorator };
    },
  ],
});
</script>

<Story name="Available Seats" args={{ course: courseWithSeats }} />

<Story name="Full Class" args={{ course: fullCourse }} />

<Story name="Low Availability" args={{ course: lowSeatsCourse }} />

<Story name="Overenrolled" args={{ course: overenrolledCourse }} />
