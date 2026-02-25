<script lang="ts">
import { cn, tooltipContentClass } from "$lib/utils";
import { Tooltip } from "bits-ui";
import type { Snippet } from "svelte";

let {
  delay = 150,
  side = "top" as "top" | "bottom" | "left" | "right",
  sideOffset = 6,
  triggerClass = "",
  contentClass = "",
  avoidCollisions = true,
  collisionPadding = 8,
  children,
  content,
}: {
  delay?: number;
  side?: "top" | "bottom" | "left" | "right";
  sideOffset?: number;
  triggerClass?: string;
  contentClass?: string;
  avoidCollisions?: boolean;
  collisionPadding?: number;
  children: Snippet;
  content: Snippet;
} = $props();

let open = $state(false);
</script>

<Tooltip.Root delayDuration={delay} disableHoverableContent={false} bind:open>
  <Tooltip.Trigger>
    {#snippet child({ props })}
      <span class={triggerClass} {...props}>
        {@render children()}
      </span>
    {/snippet}
  </Tooltip.Trigger>
  <Tooltip.Portal>
    <Tooltip.Content
      {side}
      {sideOffset}
      {avoidCollisions}
      {collisionPadding}
      class={cn(tooltipContentClass, contentClass)}
    >
      {#if open}
        {@render content()}
      {/if}
    </Tooltip.Content>
  </Tooltip.Portal>
</Tooltip.Root>
