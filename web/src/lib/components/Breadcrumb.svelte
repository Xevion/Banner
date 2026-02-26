<script lang="ts">
let {
  items,
}: {
  items: { label: string; href?: string }[];
} = $props();
</script>

<nav aria-label="Breadcrumb" class="text-sm text-muted-foreground mb-4">
  <ol class="flex items-center">
    {#each items as item, i (i)}
      <li class="inline-flex items-center">
        {#if i > 0}
          <span class="mx-1 select-none text-muted-foreground/50" aria-hidden="true">&gt;</span>
        {/if}
        {#if i === items.length - 1}
          <span class="text-foreground font-medium" aria-current="page">{item.label}</span>
        {:else if item.href}
          <a href={item.href} class="hover:text-foreground transition-colors">{item.label}</a>
        {:else}
          <span>{item.label}</span>
        {/if}
      </li>
    {/each}
  </ol>
</nav>
