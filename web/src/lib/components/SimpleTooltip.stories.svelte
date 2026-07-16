<script module>
import { defineMeta } from "@storybook/addon-svelte-csf";
import SimpleTooltip from "./SimpleTooltip.svelte";

const { Story } = defineMeta({
  title: "Components/SimpleTooltip",
  component: SimpleTooltip,
  tags: ["autodocs"],
});
</script>

<Story name="Top" args={{ text: 'This is a tooltip', side: 'top' }}>
  {#snippet template(args)}
    <SimpleTooltip {...args}>
      <button class="px-4 py-2 bg-primary text-primary-foreground rounded-md">
        Hover me
      </button>
    </SimpleTooltip>
  {/snippet}
</Story>

<Story name="Bottom" args={{ text: 'This is a tooltip', side: 'bottom' }}>
  {#snippet template(args)}
    <SimpleTooltip {...args}>
      <button class="px-4 py-2 bg-primary text-primary-foreground rounded-md">
        Hover me
      </button>
    </SimpleTooltip>
  {/snippet}
</Story>

<Story name="Left" args={{ text: 'This is a tooltip', side: 'left' }}>
  {#snippet template(args)}
    <SimpleTooltip {...args}>
      <button class="px-4 py-2 bg-primary text-primary-foreground rounded-md">
        Hover me
      </button>
    </SimpleTooltip>
  {/snippet}
</Story>

<Story name="Right" args={{ text: 'This is a tooltip', side: 'right' }}>
  {#snippet template(args)}
    <SimpleTooltip {...args}>
      <button class="px-4 py-2 bg-primary text-primary-foreground rounded-md">
        Hover me
      </button>
    </SimpleTooltip>
  {/snippet}
</Story>

<Story name="Long Text" args={{ text: 'This is a longer tooltip message\nthat spans multiple lines\nto demonstrate text wrapping', side: 'top' }}>
  {#snippet template(args)}
    <SimpleTooltip {...args}>
      <button class="px-4 py-2 bg-primary text-primary-foreground rounded-md">
        Hover me
      </button>
    </SimpleTooltip>
  {/snippet}
</Story>

<Story name="Custom Delay" args={{ text: 'This tooltip has a longer delay', side: 'top', delay: 500 }}>
  {#snippet template(args)}
    <SimpleTooltip {...args}>
      <button class="px-4 py-2 bg-primary text-primary-foreground rounded-md">
        Hover me (500ms delay)
      </button>
    </SimpleTooltip>
  {/snippet}
</Story>
