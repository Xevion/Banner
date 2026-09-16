<script lang="ts">
import { RotateCcw, TriangleAlert } from "@lucide/svelte";

interface Props {
  /** What failed, phrased for the reader rather than copied from the server. */
  title: string;
  /** The server's own explanation, shown underneath as supporting detail. */
  message?: string | null;
  /** Renders a retry button when provided. */
  onRetry?: () => void;
  /** Disables and spins the retry button while a retry is in flight. */
  retrying?: boolean;
}

let { title, message = null, onRetry, retrying = false }: Props = $props();
</script>

<div
  role="alert"
  class="flex items-start gap-3 rounded-md border border-status-red/25 bg-status-red/5 px-4 py-3 text-sm"
>
  <TriangleAlert size={16} strokeWidth={2.25} class="mt-0.5 shrink-0 text-status-red" />
  <div class="min-w-0 flex-1">
    <p class="font-medium text-status-red">{title}</p>
    {#if message}
      <p class="mt-0.5 text-xs text-muted-foreground wrap-break-word">{message}</p>
    {/if}
  </div>
  {#if onRetry}
    <button
      type="button"
      onclick={onRetry}
      disabled={retrying}
      class="shrink-0 cursor-pointer inline-flex items-center gap-1.5 rounded-md border border-status-red/30 px-2.5 py-1 text-xs font-medium text-status-red transition-colors hover:bg-status-red/10 disabled:cursor-not-allowed disabled:opacity-50"
    >
      <RotateCcw size={13} strokeWidth={2.25} class={retrying ? "animate-spin" : undefined} />
      Retry
    </button>
  {/if}
</div>
