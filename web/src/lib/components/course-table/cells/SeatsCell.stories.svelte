<script module>
import {
  courseWithSeats,
  fullCourse,
  lowSeatsCourse,
  overenrolledCourse,
} from "$lib/stories/fixtures/courses";
import { defineMeta } from "@storybook/addon-svelte-csf";
import SeatsCell from "./SeatsCell.svelte";

const { Story } = defineMeta({
  title: "Components/CourseTable/Cells/SeatsCell",
  component: SeatsCell,
  tags: ["autodocs"],
});
</script>

<Story name="Available Seats">
  <table class="text-sm">
    <tbody>
      <tr>
        <SeatsCell course={courseWithSeats} />
      </tr>
    </tbody>
  </table>
</Story>

<Story name="Full Class">
  <table class="text-sm">
    <tbody>
      <tr>
        <SeatsCell course={fullCourse} />
      </tr>
    </tbody>
  </table>
</Story>

<Story name="Low Availability">
  <table class="text-sm">
    <tbody>
      <tr>
        <SeatsCell course={lowSeatsCourse} />
      </tr>
    </tbody>
  </table>
</Story>

<Story name="Overenrolled">
  <table class="text-sm">
    <tbody>
      <tr>
        <SeatsCell course={overenrolledCourse} />
      </tr>
    </tbody>
  </table>
</Story>
