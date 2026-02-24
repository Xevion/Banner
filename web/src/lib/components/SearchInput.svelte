<script lang="ts">
import { Search, X } from "@lucide/svelte";

let {
  value = $bindable(""),
  placeholder = "Search...",
  width = "w-64",
  onSearch,
  onClear,
}: {
  value?: string;
  placeholder?: string;
  width?: string;
  onSearch: () => void;
  onClear: () => void;
} = $props();
</script>

<div class="relative">
  <Search
    size={14}
    class="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted-foreground pointer-events-none"
  />
  <input
    type="text"
    {placeholder}
    bind:value
    oninput={onSearch}
    class="bg-card border-border rounded-md border pl-8 pr-8 py-1.5 text-sm text-foreground
           placeholder:text-muted-foreground outline-none focus:ring-1 focus:ring-ring transition-shadow {width}"
  />
  {#if value}
    <button
      onclick={onClear}
      class="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors cursor-pointer"
      aria-label="Clear search"
    >
      <X size={14} />
    </button>
  {/if}
</div>
