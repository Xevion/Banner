<script module>
import { defineMeta } from "@storybook/addon-svelte-csf";
import { expect, waitFor, within } from "storybook/test";
import RangeSlider from "./RangeSlider.svelte";

const { Story } = defineMeta({
  title: "Components/RangeSlider",
  component: RangeSlider,
  tags: ["autodocs"],
  parameters: {
    layout: "padded",
  },
  argTypes: {
    min: { control: "number" },
    max: { control: "number" },
    label: { control: "text" },
    dual: { control: "boolean" },
    pips: { control: "boolean" },
    pipstep: { control: "number" },
    float: { control: "boolean" },
    hoverable: { control: "boolean" },
  },
});

/**
 * Checks the readout, which is where a set range becomes words.
 *
 * It is also the only place `formatValue` shows without hovering a handle, so a
 * story left at its default bounds renders no readout and demonstrates nothing.
 */
function expectReadout(canvasElement, text) {
  return expect(within(canvasElement).getByText(text)).toBeVisible();
}

/** Each handle announces which end it is, rather than just "slider". */
async function expectHandleLabels(canvasElement, names) {
  const canvas = within(canvasElement);
  for (const name of names) {
    const handle = canvas.getByLabelText(name);
    // Handles spring in from fully transparent, so they exist before they show.
    await waitFor(async () => await expect(handle).toBeVisible());
  }
}
</script>

<Story
  name="Default Dual"
  args={{ min: 0, max: 100, label: 'Credit Hours', valueLow: 20, valueHigh: 80 }}
  play={async ({ canvasElement }) => {
    await expectReadout(canvasElement, "20 – 80");
    await expectHandleLabels(canvasElement, ["Credit Hours minimum", "Credit Hours maximum"]);
  }}
/>

<!--
  Both handles at the bounds means no range is being asked for, so the readout
  stays away rather than restating the full span as though it were a choice.
-->
<Story
  name="At Default Bounds"
  args={{ min: 0, max: 100, label: 'Credit Hours' }}
  play={async ({ canvasElement }) => {
    await expect(within(canvasElement).queryByText(/–/)).toBeNull();
    await expect(within(canvasElement).getByText("Credit Hours")).toBeVisible();
  }}
/>

<!-- One handle, read as a ceiling rather than a span. -->
<Story
  name="Single Thumb"
  args={{ min: 0, max: 100, label: 'Maximum Credits', dual: false, value: 60 }}
  play={async ({ canvasElement }) => {
    await expectReadout(canvasElement, "≤ 60");
    await expectHandleLabels(canvasElement, ["Maximum Credits maximum"]);
  }}
/>

<Story
  name="With Pips"
  args={{ min: 0, max: 100, label: 'Credit Hours', pips: true, pipstep: 25, valueLow: 25, valueHigh: 75 }}
  play={async ({ canvasElement }) => await expectReadout(canvasElement, "25 – 75")}
/>

<!-- formatValue reaches the readout, the pips and the floating handle label. -->
<Story
  name="Custom Format"
  args={{
    min: 0,
    max: 1000,
    label: 'Budget Range',
    valueLow: 200,
    valueHigh: 800,
    formatValue: (v) => `$${v}`,
  }}
  play={async ({ canvasElement }) => await expectReadout(canvasElement, "$200 – $800")}
/>

<!-- A fractional step, where the raw numbers would read as 3 and 3.5000000001. -->
<Story
  name="GPA Range"
  args={{
    min: 0,
    max: 4,
    label: 'GPA Range',
    step: 0.1,
    valueLow: 3,
    valueHigh: 3.5,
    formatValue: (v) => v.toFixed(1),
  }}
  play={async ({ canvasElement }) => await expectReadout(canvasElement, "3.0 – 3.5")}
/>
