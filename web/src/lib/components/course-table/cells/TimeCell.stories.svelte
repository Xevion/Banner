<script module>
import {
  courseWithSeats,
  multiMeetingCourse,
  onlineCourse,
  staffInstructorCourse,
} from "$lib/stories/fixtures/courses";
import TableRowDecorator from "$lib/stories/TableRowDecorator.svelte";
import { defineMeta } from "@storybook/addon-svelte-csf";
import TimeCell from "./TimeCell.svelte";

const { Story } = defineMeta({
  title: "Components/CourseTable/Cells/TimeCell",
  component: TimeCell,
  tags: ["autodocs"],
  decorators: [
    (storyFn) => {
      storyFn();
      return { Component: TableRowDecorator };
    },
  ],
});

const weekendCourse = {
  ...staffInstructorCourse,
  meetingTimes: [
    {
      ...staffInstructorCourse.meetingTimes[0],
      timeRange: { start: "08:00", end: "14:00" },
      days: ["saturday"],
    },
  ],
};

const eveningCourse = {
  ...staffInstructorCourse,
  meetingTimes: [
    {
      ...staffInstructorCourse.meetingTimes[0],
      timeRange: { start: "19:00", end: "21:45" },
      days: ["wednesday"],
    },
  ],
};

const tbaCourse = {
  ...staffInstructorCourse,
  meetingTimes: [{ ...staffInstructorCourse.meetingTimes[0], timeRange: null, days: [] }],
};
</script>

<Story name="Standard" args={{ course: courseWithSeats }} />

<Story name="Multiple Meetings" args={{ course: multiMeetingCourse }} />

<Story name="Weekend Block" args={{ course: weekendCourse }} />

<Story name="Evening" args={{ course: eveningCourse }} />

<Story name="Async Online" args={{ course: onlineCourse }} />

<Story name="TBA" args={{ course: tbaCourse }} />
