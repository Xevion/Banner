<script module>
import { defineMeta } from "@storybook/addon-svelte-csf";
import { expect, fn, userEvent, within } from "storybook/test";
import FilterChip from "./FilterChip.svelte";

const { Story } = defineMeta({
  title: "Components/FilterChip",
  component: FilterChip,
  tags: ["autodocs"],
});
</script>

<Story
  name="Default"
  args={{ label: "Open Only", onRemove: fn() }}
  play={async ({ args, canvasElement }) => {
    const element = canvasElement;
    const canvas = within(element);
    const chip = canvas.getByRole("button", { name: /Remove Open Only filter/i });

    await expect(chip).toBeVisible();
    await userEvent.click(chip);
    // @ts-expect-error - args type not fully inferred
    await expect(args.onRemove).toHaveBeenCalled();
  }}
/>

<Story name="Long Label" args={{ label: "Computer Science", onRemove: fn() }} />

<Story name="Multiple Chips">
  {#snippet children(_args)}
    <div class="flex gap-2">
      <FilterChip label="Open Only" onRemove={fn()} />
      <FilterChip label="Computer Science" onRemove={fn()} />
      <FilterChip label="Fall 2024" onRemove={fn()} />
    </div>
  {/snippet}
</Story>
