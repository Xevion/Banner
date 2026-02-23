<script module>
import { defineMeta } from "@storybook/addon-svelte-csf";
import { expect, fn, userEvent, within } from "storybook/test";
import SegmentedChip from "./SegmentedChip.svelte";

const { Story } = defineMeta({
  title: "Components/SegmentedChip",
  component: SegmentedChip,
  tags: ["autodocs"],
});
</script>

<Story
  name="Single Segment"
  args={{ segments: ['Mon'], onRemoveSegment: fn() }}
/>

<Story
  name="Multiple Segments"
  args={{ segments: ['Mon', 'Wed', 'Fri'], onRemoveSegment: fn() }}
/>

<Story
  name="Many Segments"
  args={{ segments: ['Mon', 'Tue', 'Wed', 'Thu', 'Fri'], onRemoveSegment: fn() }}
/>

<Story
  name="Empty"
  args={{ segments: [], onRemoveSegment: fn() }}
/>

<Story
  name="Removable"
  args={{ segments: ["Mon", "Wed", "Fri"], onRemoveSegment: fn() }}
  play={async ({ args, canvasElement }) => {
    const element = canvasElement;
    const canvas = within(element);
    const segment = canvas.getByRole("button", { name: /Remove Wed filter/i });

    await expect(segment).toBeVisible();
    await userEvent.click(segment);
    // @ts-expect-error - args type not fully inferred
    await expect(args.onRemoveSegment).toHaveBeenCalledWith("Wed");
  }}
/>
