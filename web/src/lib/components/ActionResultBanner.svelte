<script lang="ts">
import { X } from "@lucide/svelte";
import { fade } from "svelte/transition";

let {
  result,
  onDismiss,
}: {
  result: { message: string; isError: boolean } | null;
  onDismiss: () => void;
} = $props();
</script>

{#if result}
  <div
    class="mb-4 rounded-md px-3 py-2 text-sm flex items-center justify-between gap-2
           {result.isError ? 'bg-destructive/10 text-destructive' : 'bg-muted text-muted-foreground'}"
    transition:fade={{ duration: 150 }}
  >
    <span>{result.message}</span>
    <button
      onclick={onDismiss}
      class="text-muted-foreground hover:text-foreground transition-colors cursor-pointer shrink-0"
      aria-label="Dismiss"
    >
      <X size={14} />
    </button>
  </div>
{/if}
