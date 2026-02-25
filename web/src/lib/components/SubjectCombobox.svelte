<script lang="ts">
import type { Subject } from "$lib/api";
import { formatNumber } from "$lib/utils";
import { Check, ChevronsUpDown } from "@lucide/svelte";
import { Combobox } from "bits-ui";
import { fly } from "svelte/transition";

let {
  subjects,
  value = $bindable(),
}: {
  subjects: Subject[];
  value: string[];
} = $props();

let open = $state(false);
let searchValue = $state("");
let containerEl = $state<HTMLDivElement>(null!);

const filteredSubjects = $derived.by(() => {
  const query = searchValue.toLowerCase().trim();
  if (query === "") return subjects;

  const exactCode: Subject[] = [];
  const codeStartsWith: Subject[] = [];
  const descriptionMatch: Subject[] = [];

  for (const s of subjects) {
    const codeLower = s.code.toLowerCase();
    const descLower = s.description.toLowerCase();

    if (codeLower === query) {
      exactCode.push(s);
    } else if (codeLower.startsWith(query)) {
      codeStartsWith.push(s);
    } else if (descLower.includes(query) || codeLower.includes(query)) {
      descriptionMatch.push(s);
    }
  }

  return [...exactCode, ...codeStartsWith, ...descriptionMatch];
});

const MAX_VISIBLE_CHIPS = 3;
const visibleChips = $derived(value.slice(0, MAX_VISIBLE_CHIPS));
const overflowCount = $derived(Math.max(0, value.length - MAX_VISIBLE_CHIPS));

function removeSubject(code: string) {
  value = value.filter((v) => v !== code);
}

// bits-ui sets the input text to the last selected item's label -- clear it
$effect(() => {
  void value;
  const input = containerEl?.querySelector("input");
  if (input) {
    input.value = "";
    searchValue = "";
  }
});
</script>

<Combobox.Root
  type="multiple"
  bind:value
  bind:open
  onOpenChange={(o: boolean) => {
    if (!o) searchValue = "";
  }}
>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="relative h-9 rounded-md border border-border bg-card
           flex flex-nowrap items-center gap-1 w-full md:w-56 pr-9 overflow-hidden cursor-pointer
           has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-ring has-[:focus-visible]:ring-offset-2 has-[:focus-visible]:ring-offset-background"
    bind:this={containerEl}
    onclick={() => { containerEl?.querySelector('input')?.focus(); }}
  >
    {#if value.length > 0}
      {#each (open ? value : visibleChips) as code (code)}
        <span
          role="button"
          tabindex="-1"
          onmousedown={(e) => { e.preventDefault(); e.stopPropagation(); }}
          onclick={(e) => { e.stopPropagation(); removeSubject(code); }}
          onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.stopPropagation(); removeSubject(code); } }}
          class="inline-flex items-center rounded bg-muted px-1.5 py-0.5 text-xs font-mono shrink-0 select-none
                 text-muted-foreground hover:bg-muted-foreground/15
                 cursor-pointer transition-colors first:ml-2"
        >
          {code}
        </span>
      {/each}
      {#if !open && overflowCount > 0}
        <span class="text-xs text-muted-foreground shrink-0 select-none">+{formatNumber(overflowCount)}</span>
      {/if}
    {/if}
    <Combobox.Input
      
      oninput={(e) => (searchValue = e.currentTarget.value)}
      onfocus={() => { open = true; }}
      class="h-full min-w-0 flex-1 bg-transparent text-muted-foreground text-sm
             placeholder:text-muted-foreground outline-none border-none
             {value.length > 0 ? 'pl-1' : 'pl-3'}"
      placeholder={value.length > 0 ? "Filter..." : "All Subjects"}
      aria-label="Search subjects"
      autocomplete="off"
      autocorrect="off"
      spellcheck={false}
    />
    <span class="absolute end-2 top-1/2 -translate-y-1/2 text-muted-foreground pointer-events-none">
      <ChevronsUpDown class="size-4" />
    </span>
  </div>
  <Combobox.Portal>
    <Combobox.Content
      customAnchor={containerEl}
      class="border border-border bg-card shadow-md
             outline-hidden z-50
             max-h-72 min-w-[var(--bits-combobox-anchor-width)] w-max max-w-96
             select-none rounded-md p-1
             data-[side=bottom]:translate-y-1 data-[side=top]:-translate-y-1"
      sideOffset={4}
      forceMount
    >
      {#snippet child({ wrapperProps, props, open: isOpen })}
        {#if isOpen}
          <div {...wrapperProps}>
            <div {...props} transition:fly={{ duration: 150, y: -4 }}>
              <Combobox.Viewport class="p-0.5">
                {#each filteredSubjects as subject (subject.code)}
                  <Combobox.Item
                    class="rounded-sm outline-hidden flex h-8 w-full select-none items-center gap-2 px-2 text-sm whitespace-nowrap
                           data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground"
                    value={subject.code}
                    label={subject.description}
                  >
                    {#snippet children({ selected })}
                      <span class="inline-flex items-center justify-center rounded bg-muted px-1 py-0.5
                                   text-xs font-mono text-muted-foreground w-10 shrink-0 text-center">
                        {subject.code}
                      </span>
                      <span class="flex-1">{subject.description}</span>
                      {#if selected}
                        <Check class="ml-auto size-4 shrink-0" />
                      {/if}
                    {/snippet}
                  </Combobox.Item>
                {:else}
                  <span class="block px-2 py-2 text-sm text-muted-foreground">
                    No subjects found.
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
