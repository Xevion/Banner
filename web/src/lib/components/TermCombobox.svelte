<script lang="ts">
import type { Term } from "$lib/api";
import { Check, ChevronsUpDown } from "@lucide/svelte";
import { Command, Popover } from "bits-ui";
import { fly } from "svelte/transition";

let {
  terms,
  value = $bindable(),
}: {
  terms: Term[];
  value: string;
} = $props();

let open = $state(false);
let searchValue = $state("");
let triggerRef = $state<HTMLDivElement>(null!);

const currentTermSlug = $derived(terms[0]?.slug ?? "");

const selectedLabel = $derived(
  terms.find((t) => t.slug === value)?.description ?? "Select term..."
);

const displayValue = $derived(open ? searchValue : selectedLabel);

const filteredTerms = $derived.by(() => {
  const query = searchValue.toLowerCase();
  const matched =
    query === "" ? terms : terms.filter((t) => t.description.toLowerCase().includes(query));

  const current = matched.find((t) => t.slug === currentTermSlug);
  const rest = matched.filter((t) => t.slug !== currentTermSlug);
  return current ? [current, ...rest] : rest;
});

function handleSelect(slug: string) {
  value = slug;
  searchValue = "";
  open = false;
}
</script>

<Command.Root shouldFilter={false} class="relative w-full md:w-40">
  <Popover.Root bind:open onOpenChange={(o) => { if (!o) searchValue = ""; }}>
    <Popover.Trigger bind:ref={triggerRef}>
      {#snippet child({ props })}
        <div
          {...props}
          onclick={(e) => {
            e.preventDefault();
            open = true;
          }}
          onkeydown={(e) => {
            if (e.key === "Enter" || e.key === " ") e.stopPropagation();
          }}
          class="relative h-9 rounded-md border border-border bg-card
                 flex items-center cursor-pointer
                 has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-ring has-[:focus-visible]:ring-offset-2 has-[:focus-visible]:ring-offset-background"
        >
          <Command.Input
            value={displayValue}
            oninput={(e: Event & { currentTarget: HTMLInputElement }) => {
              searchValue = e.currentTarget.value;
              if (!open) open = true;
            }}
            onfocus={() => { open = true; searchValue = ""; }}
            onblur={() => { setTimeout(() => { open = false; }, 150); }}
            onkeydown={(e) => {
              if (e.key === "Escape") open = false;
            }}
            class="h-full w-full bg-transparent text-muted-foreground text-sm
                   placeholder:text-muted-foreground outline-none border-none
                   pl-3 pr-9 truncate"
            placeholder="Select term..."
            aria-label="Select term"
            aria-expanded={open}
            aria-haspopup="listbox"
            autocomplete="off"
            autocorrect="off"
            spellcheck={false}
          />
          <span class="absolute end-2 top-1/2 -translate-y-1/2 text-muted-foreground pointer-events-none">
            <ChevronsUpDown class="size-4" />
          </span>
        </div>
      {/snippet}
    </Popover.Trigger>
    <Popover.Portal>
      <Popover.Content
        sideOffset={4}
        align="start"
        onOpenAutoFocus={(e) => e.preventDefault()}
        onCloseAutoFocus={(e) => e.preventDefault()}
        onInteractOutside={(e) => {
          if (triggerRef?.contains(e.target as Node)) e.preventDefault();
        }}
        forceMount
      >
        {#snippet child({ wrapperProps, props, open: isOpen })}
          {#if isOpen}
            <div {...wrapperProps}>
              <div {...props} transition:fly={{ duration: 150, y: -4 }}>
                <Command.List
                  class="border border-border bg-card shadow-md rounded-md
                         min-w-[var(--bits-popover-anchor-width)]
                         max-h-72 overflow-y-auto scrollbar-none p-1"
                >
                  {#each filteredTerms as term, i (term.slug)}
                    {#if i === 1 && term.slug !== currentTermSlug && filteredTerms[0]?.slug === currentTermSlug}
                      <div class="mx-2 my-1 h-px bg-border"></div>
                    {/if}
                    <Command.Item
                      class="rounded-sm outline-hidden flex h-8 w-full select-none items-center px-2 text-sm
                             data-[selected]:bg-accent data-[selected]:text-accent-foreground
                             {term.slug === value ? 'cursor-default' : 'cursor-pointer'}
                             {term.slug === currentTermSlug ? 'font-medium text-foreground' : 'text-foreground'}"
                      value={term.slug}
                      keywords={[term.description]}
                      onSelect={() => handleSelect(term.slug)}
                    >
                      <span class="flex-1 truncate">
                        {term.description}
                        {#if term.slug === currentTermSlug}
                          <span class="ml-1.5 text-xs text-muted-foreground font-normal">current</span>
                        {/if}
                      </span>
                      {#if term.slug === value}
                        <Check class="ml-2 size-4 shrink-0" />
                      {/if}
                    </Command.Item>
                  {:else}
                    <span class="block px-2 py-2 text-sm text-muted-foreground">
                      No terms found.
                    </span>
                  {/each}
                </Command.List>
              </div>
            </div>
          {/if}
        {/snippet}
      </Popover.Content>
    </Popover.Portal>
  </Popover.Root>
</Command.Root>
