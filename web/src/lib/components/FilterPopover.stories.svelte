<script module>
import { defineMeta } from "@storybook/addon-svelte-csf";
import FilterPopover from "./FilterPopover.svelte";

const { Story } = defineMeta({
  title: "Components/FilterPopover",
  component: FilterPopover,
  tags: ["autodocs"],
});
</script>

<Story name="Default" args={{ label: 'Filters', active: false }}>
  {#snippet template(args)}
    <FilterPopover {...args}>
      {#snippet content()}
        <p class="text-sm text-muted-foreground">Popover content</p>
      {/snippet}
    </FilterPopover>
  {/snippet}
</Story>

<Story name="Active" args={{ label: 'Filters', active: true }}>
  {#snippet template(args)}
    <FilterPopover {...args}>
      {#snippet content()}
        <p class="text-sm text-muted-foreground">Popover content</p>
      {/snippet}
    </FilterPopover>
  {/snippet}
</Story>

<Story name="With Content" args={{ label: 'Schedule', active: false }}>
  {#snippet template(args)}
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
  {/snippet}
</Story>
