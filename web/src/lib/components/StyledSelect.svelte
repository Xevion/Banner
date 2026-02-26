<script lang="ts">
import { ChevronDown, ChevronUp } from "@lucide/svelte";
import { Select } from "bits-ui";

export interface SelectItem {
  value: string;
  label: string;
}

let {
  items,
  value = $bindable(),
  onValueChange,
  triggerClass = "",
  side = "bottom",
  sideOffset = 4,
  placeholder = "Select...",
}: {
  items: SelectItem[];
  value: string;
  onValueChange?: (value: string) => void;
  triggerClass?: string;
  side?: "top" | "bottom";
  sideOffset?: number;
  placeholder?: string;
} = $props();

const selectedLabel = $derived(items.find((i) => i.value === value)?.label ?? placeholder);

function handleChange(v: string) {
  value = v;
  onValueChange?.(v);
}
</script>

<Select.Root type="single" {value} onValueChange={handleChange} {items}>
  <Select.Trigger
    class="inline-flex items-center justify-between gap-1.5 h-[30px] px-2.5
           rounded-md text-xs font-medium
           bg-muted text-muted-foreground
           hover:text-foreground transition-colors
           cursor-pointer select-none outline-none
           focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background
           {triggerClass}"
  >
    <span class="truncate max-w-[120px]">{selectedLabel}</span>
    <ChevronDown class="size-3.5 shrink-0 opacity-60" />
  </Select.Trigger>
  <Select.Portal>
    <Select.Content
      class="border border-border bg-card shadow-md outline-hidden z-50
             max-h-72 min-w-[140px] w-auto
             select-none rounded-md p-1
             data-[state=open]:animate-in data-[state=closed]:animate-out
             data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0
             data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95
             data-[side=top]:slide-in-from-bottom-2
             data-[side=bottom]:slide-in-from-top-2"
      {side}
      {sideOffset}
    >
      <Select.ScrollUpButton class="flex w-full items-center justify-center py-0.5">
        <ChevronUp class="size-3.5 text-muted-foreground" />
      </Select.ScrollUpButton>
      <Select.Viewport class="p-0.5">
        {#each items as item (item.value)}
          <Select.Item
            class="rounded-sm outline-hidden flex h-8 w-full select-none items-center
                   px-2.5 text-xs
                   data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground
                   data-[selected]:font-semibold"
            value={item.value}
            label={item.label}
          >
            {item.label}
          </Select.Item>
        {/each}
      </Select.Viewport>
      <Select.ScrollDownButton class="flex w-full items-center justify-center py-0.5">
        <ChevronDown class="size-3.5 text-muted-foreground" />
      </Select.ScrollDownButton>
    </Select.Content>
  </Select.Portal>
</Select.Root>
