<script lang="ts">
import { cn, tooltipContentClass } from "$lib/utils";
import { Tooltip } from "bits-ui";
import type { Snippet } from "svelte";

let {
  delay,
  side = "top",
  sideOffset = 6,
  passthrough = false,
  triggerClass = "",
  contentClass = "",
  portal = true,
  avoidCollisions = true,
  collisionPadding = 8,
  children,
  content,
}: {
  delay?: number;
  side?: "top" | "bottom" | "left" | "right";
  sideOffset?: number;
  passthrough?: boolean;
  triggerClass?: string;
  contentClass?: string;
  portal?: boolean;
  avoidCollisions?: boolean;
  collisionPadding?: number;
  children: Snippet;
  content: Snippet;
} = $props();
</script>

<Tooltip.Root
  {...(delay === undefined ? {} : { delayDuration: delay })}
  disableHoverableContent={passthrough}
>
  <Tooltip.Trigger>
    {#snippet child({ props })}
      <span class={triggerClass} {...props}>
        {@render children()}
      </span>
    {/snippet}
  </Tooltip.Trigger>
  {#if portal}
    <Tooltip.Portal>
      <Tooltip.Content
        {side}
        {sideOffset}
        {avoidCollisions}
        {collisionPadding}
        class={cn(tooltipContentClass, contentClass)}
      >
        {@render content()}
      </Tooltip.Content>
    </Tooltip.Portal>
  {:else}
    <Tooltip.Content
      {side}
      {sideOffset}
      {avoidCollisions}
      {collisionPadding}
      class={cn(tooltipContentClass, contentClass)}
    >
      {@render content()}
    </Tooltip.Content>
  {/if}
</Tooltip.Root>
