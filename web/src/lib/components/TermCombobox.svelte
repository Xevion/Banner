<script lang="ts">
import type { Term } from "$lib/api";
import { Check, ChevronsUpDown } from "@lucide/svelte";
import { Combobox } from "bits-ui";
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

// Left undefined until a close has to put the label back, so that the initial
// render is governed by the input's own server-rendered value.
let inputValue = $state<string | undefined>(undefined);

const currentTermSlug = $derived(terms[0]?.slug ?? "");

// Rendered as the input's initial attribute so the server emits the applied
// term, rather than the placeholder being patched away after hydration.
const selectedLabel = $derived(terms.find((t) => t.slug === value)?.description ?? "");

const filteredTerms = $derived.by(() => {
  const query = searchValue.toLowerCase();
  const matched =
    query === "" ? terms : terms.filter((t) => t.description.toLowerCase().includes(query));

  const current = matched.find((t) => t.slug === currentTermSlug);
  const rest = matched.filter((t) => t.slug !== currentTermSlug);
  return current ? [current, ...rest] : rest;
});
</script>

<Combobox.Root
  type="single"
  bind:value
  bind:open
  {inputValue}
  onOpenChangeComplete={(o) => {
    if (!o) {
      searchValue = "";
      // An abandoned search leaves its text behind, so name the selection again.
      inputValue = selectedLabel;
    }
  }}
>
  <div
    class="relative h-9 w-full md:w-40 rounded-md border border-border bg-card
           flex items-center
           has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-ring has-[:focus-visible]:ring-offset-2 has-[:focus-visible]:ring-offset-background"
  >
    <Combobox.Input
      defaultValue={selectedLabel}
      onfocus={(e) => e.currentTarget.select()}
      oninput={(e) => (searchValue = e.currentTarget.value)}
      onpointerdown={() => {
        // Deliberately not onfocus: closing returns focus to the input, so
        // opening on focus reopens the list the outside click just dismissed.
        open = true;
      }}
      class="h-full w-full bg-transparent text-muted-foreground text-sm
             placeholder:text-muted-foreground outline-none border-none
             pl-3 pr-9 truncate cursor-pointer"
      placeholder="Select term..."
      aria-label="Select term"
      autocomplete="off"
      autocorrect="off"
      spellcheck={false}
    />
    <Combobox.Trigger
      aria-label="Show terms"
      class="absolute end-2 top-1/2 -translate-y-1/2 text-muted-foreground cursor-pointer"
    >
      <ChevronsUpDown class="size-4" />
    </Combobox.Trigger>
  </div>
  <Combobox.Portal>
    <Combobox.Content sideOffset={4} align="start" forceMount>
      {#snippet child({ wrapperProps, props, open: isOpen })}
        {#if isOpen}
          <div {...wrapperProps}>
            <div {...props} transition:fly={{ duration: 150, y: -4 }}>
              <Combobox.Viewport
                class="border border-border bg-card shadow-md rounded-md
                       min-w-[var(--bits-combobox-anchor-width)]
                       max-h-72 overflow-y-auto scrollbar-none p-1"
              >
                {#each filteredTerms as term, i (term.slug)}
                  {#if i === 1 && filteredTerms[0]?.slug === currentTermSlug}
                    <div class="mx-2 my-1 h-px bg-border"></div>
                  {/if}
                  <Combobox.Item
                    class="rounded-sm outline-hidden flex h-8 w-full select-none items-center px-2 text-sm
                           data-highlighted:bg-accent data-highlighted:text-accent-foreground
                           {term.slug === value ? 'cursor-default' : 'cursor-pointer'}
                           {term.slug === currentTermSlug ? 'font-medium text-foreground' : 'text-foreground'}"
                    value={term.slug}
                    label={term.description}
                  >
                    {#snippet children({ selected })}
                      <span class="flex-1 truncate">
                        {term.description}
                        {#if term.slug === currentTermSlug}
                          <span class="ml-1.5 text-xs text-muted-foreground font-normal">current</span>
                        {/if}
                      </span>
                      {#if selected}
                        <Check class="ml-2 size-4 shrink-0" />
                      {/if}
                    {/snippet}
                  </Combobox.Item>
                {:else}
                  <span class="block px-2 py-2 text-sm text-muted-foreground">
                    No terms found.
                  </span>
                {/each}
              </Combobox.Viewport>
            </div>
          </div>
        {/if}
      {/snippet}
    </Combobox.Content>
  </Combobox.Portal>
</Combobox.Root>
