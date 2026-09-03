<script lang="ts">
import SearchInput from "$lib/components/SearchInput.svelte";
import type { useDebounceSearch } from "$lib/composables";
import { RefreshCw } from "@lucide/svelte";

let {
  title,
  searchPlaceholder,
  search,
  actionLabel,
  actionLoading,
  onAction,
}: {
  title: string;
  searchPlaceholder: string;
  search: ReturnType<typeof useDebounceSearch>;
  actionLabel: string;
  actionLoading: boolean;
  onAction: () => void;
} = $props();
</script>

<div class="flex items-center gap-3 mb-4">
  <h1 class="text-lg font-semibold text-foreground">{title}</h1>
  <div class="flex-1"></div>

  <SearchInput
    bind:value={search.input}
    placeholder={searchPlaceholder}
    onSearch={search.trigger}
    onClear={() => search.clear()}
  />

  <button
    onclick={onAction}
    disabled={actionLoading}
    class="inline-flex items-center gap-1.5 rounded-md bg-muted px-3 py-1.5 text-sm font-medium
           text-foreground hover:bg-accent transition-colors disabled:opacity-50 cursor-pointer"
  >
    <RefreshCw size={14} class={actionLoading ? "animate-spin" : ""} />
    {actionLabel}
  </button>
</div>
