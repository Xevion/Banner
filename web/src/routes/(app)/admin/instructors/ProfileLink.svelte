<script lang="ts">
import { cn } from "$lib/utils";
import { ExternalLink } from "@lucide/svelte";

let {
  instructorId,
  label = "Profile",
  class: className = "",
}: {
  instructorId: number;
  label?: string;
  class?: string;
} = $props();
</script>

<!-- A new tab, and no click reaching the row: neither may cost the admin their review state. -->
<a
  href="/instructors/{instructorId}"
  target="_blank"
  rel="noopener"
  aria-label="Open public profile in a new tab"
  title="Open public profile in a new tab"
  onclick={(e) => e.stopPropagation()}
  class={cn(
    "inline-flex shrink-0 items-center gap-1 text-xs text-muted-foreground",
    "hover:text-foreground transition-colors",
    className
  )}
>
  {label}
  <ExternalLink class="size-3" />
</a>
