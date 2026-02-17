<script lang="ts">
import type { ColumnVisibilityController } from "$lib/composables/useColumnVisibility.svelte";
import { Check, Columns3, RotateCcw } from "@lucide/svelte";
import { DropdownMenu } from "bits-ui";
import { fly } from "svelte/transition";

let { columns }: { columns: ColumnVisibilityController } = $props();
</script>

<DropdownMenu.Root>
  <DropdownMenu.Trigger
    class="inline-flex items-center gap-1.5 rounded-md border border-border bg-background px-2.5 py-1.5 text-xs font-medium text-muted-foreground hover:bg-accent hover:text-accent-foreground transition-colors cursor-pointer select-none shrink-0"
  >
    <Columns3 class="size-3.5" />
    View
  </DropdownMenu.Trigger>
  <DropdownMenu.Portal>
    <DropdownMenu.Content
      class="z-50 min-w-40 rounded-md border border-border bg-card p-1 text-card-foreground shadow-lg"
      align="end"
      sideOffset={4}
      forceMount
    >
      {#snippet child({ wrapperProps, props, open })}
        {#if open}
          <div {...wrapperProps}>
            <div
              {...props}
              transition:fly={{
                duration: 150,
                y: -10,
              }}
            >
              <DropdownMenu.Group>
                <DropdownMenu.GroupHeading
                  class="px-2 py-1.5 text-xs font-medium text-muted-foreground select-none"
                >
                  Toggle columns
                </DropdownMenu.GroupHeading>
                {#each columns.columns as col (col.id)}
                  <DropdownMenu.CheckboxItem
                    checked={columns.visibility[col.id] !== false}
                    closeOnSelect={false}
                    onCheckedChange={(checked) => {
                      columns.toggle(col.id, checked);
                    }}
                    class="relative flex items-center gap-2 rounded-sm px-2 py-1.5 text-sm cursor-pointer select-none outline-none data-highlighted:bg-accent data-highlighted:text-accent-foreground"
                  >
                    {#snippet children({ checked })}
                      <span
                        class="flex size-4 items-center justify-center rounded-sm border border-border"
                      >
                        {#if checked}
                          <Check class="size-3" />
                        {/if}
                      </span>
                      {col.label}
                    {/snippet}
                  </DropdownMenu.CheckboxItem>
                {/each}
              </DropdownMenu.Group>
              {#if columns.hasCustomVisibility}
                <DropdownMenu.Separator class="mx-1 my-1 h-px bg-border" />
                <DropdownMenu.Item
                  class="flex items-center gap-2 rounded-sm px-2 py-1.5 text-sm cursor-pointer select-none outline-none data-highlighted:bg-accent data-highlighted:text-accent-foreground"
                  onSelect={columns.reset}
                >
                  <RotateCcw class="size-3.5" />
                  Reset to default
                </DropdownMenu.Item>
              {/if}
            </div>
          </div>
        {/if}
      {/snippet}
    </DropdownMenu.Content>
  </DropdownMenu.Portal>
</DropdownMenu.Root>
