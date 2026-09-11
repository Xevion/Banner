<script module>
import { CAMPUS_GROUPS } from "$lib/labels";
import { defineMeta } from "@storybook/addon-svelte-csf";
import { expect, within } from "storybook/test";
import AvailabilityFilter from "./AvailabilityFilter.svelte";

const { Story } = defineMeta({
  title: "Components/AvailabilityFilter",
  component: AvailabilityFilter,
  tags: ["autodocs"],
});

/** Asserts the pill this story is named after is the one showing as chosen. */
function expectPressed(canvasElement, name) {
  const canvas = within(canvasElement);
  return expect(canvas.getByRole("button", { name })).toHaveAttribute("aria-pressed", "true");
}
</script>

<!--
  The campus codes come from the same table the component reads, because this
  widget shows a selection only when the codes it is given fall in one of those
  groups. Spelling them out here instead let all three stories render the same
  unselected state, since none of the invented codes matched anything.
-->

<Story name="Default" args={{ campus: [] }} />

<Story
  name="Campus Selected"
  args={{ campus: [...CAMPUS_GROUPS.campusStudents] }}
  play={async ({ canvasElement }) => await expectPressed(canvasElement, /Campus Students/)}
/>

<Story
  name="Online Selected"
  args={{ campus: [...CAMPUS_GROUPS.onlinePrograms] }}
  play={async ({ canvasElement }) => await expectPressed(canvasElement, /Online Programs/)}
/>

<!--
  Reachable from a link naming campuses from both groups. It is not the same as
  no filter, which asks for every campus including any in neither group, so both
  pills light rather than the widget claiming nothing is filtered.
-->
<Story
  name="Both Groups"
  args={{ campus: [...CAMPUS_GROUPS.campusStudents, ...CAMPUS_GROUPS.onlinePrograms] }}
  play={async ({ canvasElement }) => {
    await expectPressed(canvasElement, /Campus Students/);
    await expectPressed(canvasElement, /Online Programs/);
  }}
/>

<!-- Nothing filtered is the one state where neither pill is lit. -->
<Story
  name="Nothing Selected"
  args={{ campus: [] }}
  play={async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    for (const name of [/Campus Students/, /Online Programs/]) {
      await expect(canvas.getByRole("button", { name })).toHaveAttribute("aria-pressed", "false");
    }
  }}
/>
