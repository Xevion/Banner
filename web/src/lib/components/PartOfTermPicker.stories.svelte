<script module>
import { defineMeta } from "@storybook/addon-svelte-csf";
import { expect, within } from "storybook/test";
import PartOfTermPicker from "./PartOfTermPicker.svelte";

const mockPartsOfTerm = [
  { filterValue: "1", description: "Full Term" },
  { filterValue: "8W1", description: "First 8 Week" },
  { filterValue: "8W2", description: "Second 8 Week" },
  { filterValue: "SUM1", description: "Summer 1" },
  { filterValue: "SUM2", description: "Summer 2" },
];

const { Story } = defineMeta({
  title: "Components/PartOfTermPicker",
  component: PartOfTermPicker,
  tags: ["autodocs"],
});

function pressedCount(canvasElement) {
  return within(canvasElement)
    .getAllByRole("button")
    .filter((b) => b.getAttribute("aria-pressed") === "true").length;
}
</script>

<Story
  name="Default"
  args={{ partOfTerm: [], partsOfTerm: mockPartsOfTerm }}
  play={async ({ canvasElement }) => await expect(pressedCount(canvasElement)).toBe(0)}
/>

<Story
  name="Some Selected"
  args={{ partOfTerm: ['1', '8W1'], partsOfTerm: mockPartsOfTerm }}
  play={async ({ canvasElement }) => await expect(pressedCount(canvasElement)).toBe(2)}
/>

<!--
  The narrow frame is the point: mobile wraps its pills onto more than one row
  and rounds them fully, neither of which shows at a width where nothing has to
  wrap.
-->
<Story name="Mobile Layout" args={{ partOfTerm: ['8W1'], partsOfTerm: mockPartsOfTerm, mobile: true }}>
  {#snippet template(args)}
    <div class="w-56">
      <PartOfTermPicker {...args} />
    </div>
  {/snippet}
</Story>

<!-- The same options at desktop width, for comparison with the story above. -->
<Story name="Desktop Layout" args={{ partOfTerm: ['8W1'], partsOfTerm: mockPartsOfTerm }}>
  {#snippet template(args)}
    <div class="w-[32rem]">
      <PartOfTermPicker {...args} />
    </div>
  {/snippet}
</Story>

<!--
  Some terms are not divided at all. The picker used to render nothing here,
  leaving a gap in the filter panel that reads as something having gone wrong.
-->
<Story
  name="Empty Options"
  args={{ partOfTerm: [], partsOfTerm: [] }}
  play={async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("Part of Term")).toBeVisible();
    await expect(canvas.getByText(/not divided into parts/)).toBeVisible();
  }}
/>
