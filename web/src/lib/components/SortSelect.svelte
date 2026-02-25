<script lang="ts">
import { ArrowUp } from "@lucide/svelte";
import { Select } from "bits-ui";

export interface SortOption {
  value: string;
  label: string;
  /** Direction to default to when this field is first selected. */
  defaultDirection?: "asc" | "desc";
}

let {
  options,
  value = $bindable(),
}: {
  options: SortOption[];
  value: string;
} = $props();

const field = $derived(
  value.endsWith("_asc") ? value.slice(0, -4) : value.endsWith("_desc") ? value.slice(0, -5) : value
);

const direction = $derived<"asc" | "desc">(value.endsWith("_desc") ? "desc" : "asc");

const fieldLabel = $derived(options.find((o) => o.value === field)?.label ?? "Sort");

function onFieldChange(newField: string) {
  if (!newField) return;
  const opt = options.find((o) => o.value === newField);
  const newDir = opt?.defaultDirection ?? direction;
  value = `${newField}_${newDir}`;
}

function toggleDirection() {
  const newDir = direction === "asc" ? "desc" : "asc";
  value = `${field}_${newDir}`;
}
</script>

<div class="flex items-center">
	<Select.Root type="single" value={field} onValueChange={onFieldChange} items={options}>
		<Select.Trigger
			class="inline-flex items-center justify-between gap-1.5 h-9 px-3
			       rounded-md rounded-r-none border border-r-0 border-border bg-card
			       text-sm text-muted-foreground min-w-32
			       hover:bg-muted/50 transition-colors cursor-pointer select-none outline-none
			       focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background"
		>
			{fieldLabel}
		</Select.Trigger>
		<Select.Portal>
			<Select.Content
				class="border border-border bg-card shadow-md outline-hidden z-50
				       min-w-36 select-none rounded-md p-1
				       data-[state=open]:animate-in data-[state=closed]:animate-out
				       data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0
				       data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95
				       data-[side=bottom]:slide-in-from-top-2"
				sideOffset={4}
			>
				<Select.Viewport class="p-0.5">
					{#each options as opt (opt.value)}
						<Select.Item
							class="rounded-sm outline-hidden flex h-8 w-full select-none items-center
							       px-2.5 text-sm cursor-pointer
							       data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground
							       data-[selected]:font-medium"
							value={opt.value}
							label={opt.label}
						>
							{opt.label}
						</Select.Item>
					{/each}
				</Select.Viewport>
			</Select.Content>
		</Select.Portal>
	</Select.Root>

	<button
		type="button"
		onclick={toggleDirection}
		aria-label={direction === "asc" ? "Sort ascending" : "Sort descending"}
		class="inline-flex items-center justify-center h-9 w-9
		       rounded-md rounded-l-none border border-border bg-card
		       text-muted-foreground
		       hover:bg-muted/50 transition-colors cursor-pointer select-none outline-none
		       focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background"
	>
		<ArrowUp
			class="size-4 transition-transform duration-200 {direction === 'desc' ? 'rotate-180' : ''}"
		/>
	</button>
</div>
