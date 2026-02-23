<script module>
import { defineMeta } from "@storybook/addon-svelte-csf";
import { expect, fn, userEvent, within } from "storybook/test";
import TogglePill from "./TogglePill.svelte";

const { Story } = defineMeta({
  title: "Components/TogglePill",
  component: TogglePill,
  tags: ["autodocs"],
});
</script>

<Story
  name="Default"
  args={{ label: 'Filter', active: false, onclick: fn() }}
/>

<Story
  name="Active"
  args={{ label: 'Filter', active: true, onclick: fn() }}
/>

<Story
  name="Rounded MD"
  args={{ label: 'Filter', active: false, variant: 'rounded-md', onclick: fn() }}
/>

<Story
  name="Toggleable"
  args={{ label: "Toggle Me", active: false, onclick: fn() }}
  play={async ({ args, canvasElement }) => {
    const element = canvasElement;
    const canvas = within(element);
    const button = canvas.getByRole("button", { name: /Toggle Me/i });

    await expect(button).toHaveAttribute("aria-pressed", "false");
    await userEvent.click(button);
    // @ts-expect-error - args type not fully inferred
    await expect(args.onclick).toHaveBeenCalled();
  }}
/>
