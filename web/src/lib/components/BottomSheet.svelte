<script lang="ts">
import type { Snippet } from "svelte";
import { fade, fly } from "svelte/transition";

const DISMISS_THRESHOLD = 100;

let {
  open = $bindable(false),
  maxHeight = "80vh",
  label,
  children,
}: {
  open: boolean;
  maxHeight?: string;
  label?: string;
  children: Snippet;
} = $props();

let dragOffset = $state(0);
let dragging = $state(false);
let dragStartY = 0;

function close() {
  open = false;
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") close();
}

function onPointerDown(e: PointerEvent) {
  dragging = true;
  dragStartY = e.clientY;
  dragOffset = 0;
  (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
}

function onPointerMove(e: PointerEvent) {
  if (!dragging) return;
  const delta = e.clientY - dragStartY;
  dragOffset = Math.max(0, delta);
}

function onPointerUp() {
  if (!dragging) return;
  dragging = false;
  if (dragOffset > DISMISS_THRESHOLD) {
    close();
  }
  dragOffset = 0;
}

$effect(() => {
  if (open) {
    const prev = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = prev;
    };
  }
});
</script>

<svelte:window onkeydown={onKeydown} />

{#if open}
  <!-- Backdrop -->
  <div
    class="fixed inset-0 z-40 bg-black/40"
    role="presentation"
    transition:fade={{ duration: 200 }}
    onclick={close}
  ></div>

  <!-- Sheet -->
  <div
    class="fixed inset-x-0 bottom-0 z-50 flex flex-col rounded-t-2xl border-t border-border bg-background shadow-[0_-4px_20px_rgba(0,0,0,0.1)] pb-[env(safe-area-inset-bottom)]"
    style="max-height: {maxHeight}; transform: translateY({dragOffset}px);"
    class:transition-transform={!dragging}
    class:duration-250={!dragging}
    class:ease-out={!dragging}
    transition:fly={{ y: 300, duration: 250 }}
    role="dialog"
    aria-modal="true"
    aria-label={label}
  >
    <!-- Drag handle -->
    <div
      class="flex shrink-0 cursor-grab items-center justify-center py-3 touch-none"
      class:cursor-grabbing={dragging}
      role="separator"
      onpointerdown={onPointerDown}
      onpointermove={onPointerMove}
      onpointerup={onPointerUp}
      onpointercancel={onPointerUp}
    >
      <div class="h-1 w-10 rounded-full bg-muted-foreground/30"></div>
    </div>

    <!-- Content -->
    <div class="overflow-y-auto">
      {@render children()}
    </div>
  </div>
{/if}
