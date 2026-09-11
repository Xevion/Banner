<script module>
import { defineMeta } from "@storybook/addon-svelte-csf";
import { expect, userEvent, waitFor, within } from "storybook/test";
import FilterPopover from "./FilterPopover.svelte";

const { Story } = defineMeta({
  title: "Components/FilterPopover",
  component: FilterPopover,
  tags: ["autodocs"],
});

/**
 * Reserves room under the trigger for the open panel.
 *
 * The panel is portalled and positioned absolutely, so it adds no height of its
 * own and the story frame would otherwise be shorter than its own content.
 */
const ROOM_FOR_PANEL = "min-h-56";

/** Opens the popover, whose content is portalled out of the story canvas. */
async function open(canvasElement, label) {
  const canvas = within(canvasElement);
  await userEvent.click(canvas.getByRole("button", { name: `${label} filters` }));
  return within(document.body);
}
</script>

<Story name="Default" args={{ label: 'Filters', active: false }}>
  {#snippet template(args)}
    <div class={ROOM_FOR_PANEL}>
      <FilterPopover {...args}>
      {#snippet content()}
        <p class="text-sm text-muted-foreground">Popover content</p>
      {/snippet}
      </FilterPopover>
    </div>
  {/snippet}
</Story>

<!-- Active swaps the chevron for a dot, so the trigger alone shows the state. -->
<Story
  name="Active"
  args={{ label: 'Filters', active: true }}
  play={async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("button", { name: /Filters filters/ })).toHaveClass(/border-primary/);
  }}
>
  {#snippet template(args)}
    <div class={ROOM_FOR_PANEL}>
      <FilterPopover {...args}>
      {#snippet content()}
        <p class="text-sm text-muted-foreground">Popover content</p>
      {/snippet}
      </FilterPopover>
    </div>
  {/snippet}
</Story>

<Story
  name="With Content"
  args={{ label: 'Schedule', active: false }}
  parameters={{ docs: { story: { autoplay: true } } }}
  play={async ({ canvasElement }) => {
    // Closed, this story is indistinguishable from Default; the content it is
    // named for only exists once the trigger has been clicked.
    const popover = await open(canvasElement, "Schedule");
    // The panel flies in over 150ms, so it is in the DOM before it is on screen.
    await waitFor(async () => {
      await expect(popover.getByText("Filter options go here")).toBeVisible();
    });
    await expect(popover.getByRole("button", { name: "Apply" })).toBeVisible();
  }}
>
  {#snippet template(args)}
    <div class={ROOM_FOR_PANEL}>
      <FilterPopover {...args}>
      {#snippet content()}
        <div class="flex flex-col gap-2">
          <p class="text-sm text-foreground">Filter options go here</p>
          <button class="px-3 py-1.5 bg-primary text-primary-foreground rounded-md text-sm">
            Apply
          </button>
        </div>
      {/snippet}
      </FilterPopover>
    </div>
  {/snippet}
</Story>
