<script module>
import { defineMeta } from "@storybook/addon-svelte-csf";
import { expect, userEvent, within } from "storybook/test";
import SimpleTooltip from "./SimpleTooltip.svelte";

const { Story } = defineMeta({
  title: "Components/SimpleTooltip",
  component: SimpleTooltip,
  tags: ["autodocs"],
  parameters: {
    docs: {
      // Without this the docs page shows six identical buttons: the thing each
      // story is about only exists once the trigger has been hovered.
      story: { autoplay: true, height: "180px" },
    },
  },
});

/** Hovers the trigger and waits for the tooltip to be placed. */
async function open(canvasElement) {
  const trigger = canvasElement.querySelector("[data-tooltip-trigger]");
  await userEvent.hover(trigger);
  // Portalled out of the story canvas, and delayed before it opens.
  return await within(document.body).findByText(/tooltip/i, { selector: "[data-tooltip-content]" });
}

/** Opens the tooltip and checks it landed on the side the story is named for. */
async function expectSide(canvasElement, side) {
  const content = await open(canvasElement);
  await expect(content).toHaveAttribute("data-side", side);
}
</script>

{#snippet trigger(args)}
  <SimpleTooltip {...args}>
    <button class="px-4 py-2 bg-primary text-primary-foreground rounded-md"> Hover me </button>
  </SimpleTooltip>
{/snippet}

<Story
  name="Top"
  args={{ text: 'This is a tooltip', side: 'top' }}
  template={trigger}
  play={async ({ canvasElement }) => await expectSide(canvasElement, "top")}
/>

<Story
  name="Bottom"
  args={{ text: 'This is a tooltip', side: 'bottom' }}
  template={trigger}
  play={async ({ canvasElement }) => await expectSide(canvasElement, "bottom")}
/>

<Story
  name="Left"
  args={{ text: 'This is a tooltip', side: 'left' }}
  template={trigger}
  play={async ({ canvasElement }) => await expectSide(canvasElement, "left")}
/>

<Story
  name="Right"
  args={{ text: 'This is a tooltip', side: 'right' }}
  template={trigger}
  play={async ({ canvasElement }) => await expectSide(canvasElement, "right")}
/>

<Story
  name="Long Text"
  args={{
    text: 'This is a longer tooltip message\nthat spans multiple lines\nto demonstrate text wrapping',
    side: 'top',
  }}
  template={trigger}
  play={async ({ canvasElement }) => {
    const content = await open(canvasElement);
    // The newlines are meant to survive, which is what whitespace-pre-line buys.
    await expect(content).toHaveTextContent(/spans multiple lines/);
  }}
/>

<Story
  name="Custom Delay"
  args={{ text: 'This tooltip waits half a second', side: 'top', delay: 500 }}
  template={trigger}
  play={async ({ canvasElement }) => await expect(await open(canvasElement)).toBeVisible()}
/>
