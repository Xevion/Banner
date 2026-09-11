<script module>
import { DAY_OPTIONS } from "$lib/days";
import { defineMeta } from "@storybook/addon-svelte-csf";
import { expect, within } from "storybook/test";
import DaysOfWeekPicker from "./DaysOfWeekPicker.svelte";

const { Story } = defineMeta({
  title: "Components/DaysOfWeekPicker",
  component: DaysOfWeekPicker,
  tags: ["autodocs"],
});

const everyDay = DAY_OPTIONS.map((d) => d.value);

/** How many pills are showing as chosen, whatever the picker offers. */
function pressedCount(canvasElement) {
  return within(canvasElement)
    .getAllByRole("button")
    .filter((b) => b.getAttribute("aria-pressed") === "true").length;
}
</script>

<Story
  name="Default"
  args={{ days: [] }}
  play={async ({ canvasElement }) => await expect(pressedCount(canvasElement)).toBe(0)}
/>

<Story
  name="Some Selected"
  args={{ days: ['monday', 'wednesday', 'friday'] }}
  play={async ({ canvasElement }) => await expect(pressedCount(canvasElement)).toBe(3)}
/>

<!--
  Taken from the same list the picker renders, so adding a day there cannot
  leave this story showing some of them switched off under this name.
-->
<Story
  name="All Selected"
  args={{ days: everyDay }}
  play={async ({ canvasElement }) =>
    await expect(pressedCount(canvasElement)).toBe(DAY_OPTIONS.length)}
/>

<Story
  name="Weekdays Only"
  args={{ days: ['monday', 'tuesday', 'wednesday', 'thursday', 'friday'] }}
  play={async ({ canvasElement }) => await expect(pressedCount(canvasElement)).toBe(5)}
/>

<!--
  Narrow on purpose: mobile drops the even-width stretch and the nowrap, which
  only tells against a width the pills cannot all sit on.
-->
<Story name="Mobile Layout" args={{ days: ['monday', 'wednesday'], mobile: true }}>
  {#snippet template(args)}
    <div class="w-56">
      <DaysOfWeekPicker {...args} />
    </div>
  {/snippet}
</Story>

<!-- The same days at desktop width, for comparison with the story above. -->
<Story name="Desktop Layout" args={{ days: ['monday', 'wednesday'] }}>
  {#snippet template(args)}
    <div class="w-[32rem]">
      <DaysOfWeekPicker {...args} />
    </div>
  {/snippet}
</Story>
