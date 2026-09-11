<script module>
import { defineMeta } from "@storybook/addon-svelte-csf";
import { expect, within } from "storybook/test";
import TimeRangeInput from "./TimeRangeInput.svelte";

const { Story } = defineMeta({
  title: "Components/TimeRangeInput",
  component: TimeRangeInput,
  tags: ["autodocs"],
});

/**
 * Times are held as "HHMM" and shown in the form a person would type.
 *
 * Passing "09:00" instead renders an empty box, because the formatter takes the
 * stored form and rejects anything that is not four digits.
 */
function expectTimes(canvasElement, start, end) {
  const canvas = within(canvasElement);
  return Promise.all([
    expect(canvas.getByLabelText("Earliest start time")).toHaveValue(start),
    expect(canvas.getByLabelText("Latest end time")).toHaveValue(end),
  ]);
}
</script>

<Story
  name="Default"
  args={{ timeStart: null, timeEnd: null }}
  play={async ({ canvasElement }) => await expectTimes(canvasElement, "", "")}
/>

<Story
  name="With Start Time"
  args={{ timeStart: '0900', timeEnd: null }}
  play={async ({ canvasElement }) => await expectTimes(canvasElement, "9:00 AM", "")}
/>

<Story
  name="With End Time"
  args={{ timeStart: null, timeEnd: '1700' }}
  play={async ({ canvasElement }) => await expectTimes(canvasElement, "", "5:00 PM")}
/>

<Story
  name="Full Range"
  args={{ timeStart: '0800', timeEnd: '1400' }}
  play={async ({ canvasElement }) => await expectTimes(canvasElement, "8:00 AM", "2:00 PM")}
/>

<!-- Midnight and noon are where a 12-hour clock usually goes wrong. -->
<Story
  name="Midnight to Noon"
  args={{ timeStart: '0000', timeEnd: '1200' }}
  play={async ({ canvasElement }) => await expectTimes(canvasElement, "12:00 AM", "12:00 PM")}
/>
