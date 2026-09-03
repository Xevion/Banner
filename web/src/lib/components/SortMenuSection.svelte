<script lang="ts">
import { ArrowDown, ArrowUp, Check, ChevronRight, Plus, X } from "@lucide/svelte";
import { DropdownMenu } from "bits-ui";
import type { SortController } from "$lib/composables/useSort.svelte";

// bits-ui re-exports one set of menu parts under both DropdownMenu and
// ContextMenu, so this section renders unchanged inside either menu.
let { sort }: { sort: SortController } = $props();

const itemClass =
  "relative flex items-center gap-2 rounded-sm px-2 py-1.5 text-sm cursor-pointer select-none outline-none data-highlighted:bg-accent data-highlighted:text-accent-foreground data-disabled:cursor-default data-disabled:opacity-50";
const contentClass =
  "z-50 min-w-44 rounded-md border border-border bg-card p-1 text-card-foreground shadow-lg";
const headingClass = "px-2 py-1.5 text-xs font-medium text-muted-foreground select-none";
</script>

<DropdownMenu.Group>
  <DropdownMenu.GroupHeading class={headingClass}>Sort</DropdownMenu.GroupHeading>

  {#each sort.terms as term, index (term.key)}
    <DropdownMenu.Sub>
      <DropdownMenu.SubTrigger class={itemClass}>
        <span class="w-3 shrink-0 text-xs tabular-nums text-muted-foreground">{index + 1}</span>
        {#if term.desc}
          <ArrowDown class="size-3.5 shrink-0 text-muted-foreground" />
        {:else}
          <ArrowUp class="size-3.5 shrink-0 text-muted-foreground" />
        {/if}
        <span class="flex-1 truncate">{sort.labelOf(term)}</span>
        <ChevronRight class="size-3.5 shrink-0 text-muted-foreground" />
      </DropdownMenu.SubTrigger>
      <DropdownMenu.SubContent class={contentClass} sideOffset={4}>
        <DropdownMenu.Item
          class={itemClass}
          closeOnSelect={false}
          onSelect={() => sort.setDirection(term.key, false)}
        >
          <span class="flex size-3.5 shrink-0 items-center justify-center">
            {#if !term.desc}
              <Check class="size-3" />
            {/if}
          </span>
          {sort.label(term.key, false)}
        </DropdownMenu.Item>
        <DropdownMenu.Item
          class={itemClass}
          closeOnSelect={false}
          onSelect={() => sort.setDirection(term.key, true)}
        >
          <span class="flex size-3.5 shrink-0 items-center justify-center">
            {#if term.desc}
              <Check class="size-3" />
            {/if}
          </span>
          {sort.label(term.key, true)}
        </DropdownMenu.Item>
        <DropdownMenu.Separator class="mx-1 my-1 h-px bg-border" />
        <DropdownMenu.Item
          class={itemClass}
          closeOnSelect={false}
          disabled={index === 0}
          onSelect={() => sort.move(term.key, "up")}
        >
          <ArrowUp class="size-3.5 shrink-0" />
          Move up
        </DropdownMenu.Item>
        <DropdownMenu.Item
          class={itemClass}
          closeOnSelect={false}
          disabled={index === sort.terms.length - 1}
          onSelect={() => sort.move(term.key, "down")}
        >
          <ArrowDown class="size-3.5 shrink-0" />
          Move down
        </DropdownMenu.Item>
        <DropdownMenu.Separator class="mx-1 my-1 h-px bg-border" />
        <!-- Closes the menu: removing a term unmounts the submenu this sits in. -->
        <DropdownMenu.Item class={itemClass} onSelect={() => sort.remove(term.key)}>
          <X class="size-3.5 shrink-0" />
          Remove
        </DropdownMenu.Item>
      </DropdownMenu.SubContent>
    </DropdownMenu.Sub>
  {/each}

  <DropdownMenu.Sub>
    <DropdownMenu.SubTrigger class={itemClass}>
      <Plus class="size-3.5 shrink-0 text-muted-foreground" />
      <span class="flex-1">{sort.isEmpty ? "Sort by" : "Add tiebreaker"}</span>
      <ChevronRight class="size-3.5 shrink-0 text-muted-foreground" />
    </DropdownMenu.SubTrigger>
    <DropdownMenu.SubContent class={contentClass} sideOffset={4}>
      {#if sort.isFull}
        <!-- Stated rather than hidden: the trigger stays enabled so an open
             submenu is never disabled out from under the keyboard. -->
        <DropdownMenu.Item class={itemClass} disabled>
          Limit of {sort.maxTerms} keys reached
        </DropdownMenu.Item>
      {:else}
        <!-- One entry per key rather than two: the other way round is a click
             away in the term's own submenu, and this list stays readable. -->
        {#each sort.available as option (option.key)}
          <DropdownMenu.Item
            class={itemClass}
            closeOnSelect={false}
            onSelect={() => sort.append({ key: option.key, desc: false })}
          >
            {option.ascLabel}
          </DropdownMenu.Item>
        {/each}
      {/if}
    </DropdownMenu.SubContent>
  </DropdownMenu.Sub>

  {#if !sort.isEmpty}
    <DropdownMenu.Item class={itemClass} onSelect={sort.clear}>
      <X class="size-3.5 shrink-0" />
      Clear sort
    </DropdownMenu.Item>
  {/if}
</DropdownMenu.Group>
