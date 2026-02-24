<script lang="ts">
import { formatNumber } from "$lib/utils";
import { ChevronDown, ChevronLeft, ChevronRight, ChevronUp } from "@lucide/svelte";
import { Select } from "bits-ui";
import type { Action } from "svelte/action";

const slideIn: Action<HTMLElement, number> = (node, direction) => {
  if (direction !== 0) {
    node.animate(
      [
        {
          transform: `translateX(${direction * 20}px)`,
          opacity: 0,
        },
        { transform: "translateX(0)", opacity: 1 },
      ],
      { duration: 200, easing: "ease-out" }
    );
  }
};

let {
  variant = "default",
  totalCount,
  onPageChange,
  // default variant
  offset = 0,
  limit = 25,
  loading = false,
  // simple variant (renamed to avoid conflict with derived currentPage)
  currentPage: simplePage = 1,
  perPage = 25,
}: {
  variant?: "default" | "simple";
  totalCount: number;
  onPageChange: (value: number) => void;
  // default
  offset?: number;
  limit?: number;
  loading?: boolean;
  // simple
  currentPage?: number;
  perPage?: number;
} = $props();

const currentPage = $derived(Math.floor(offset / limit) + 1);
const totalPages = $derived(Math.ceil(totalCount / limit));
const start = $derived(offset + 1);
const end = $derived(Math.min(offset + limit, totalCount));

// Track direction for slide animation
let direction = $state(0);

// 5 page slots: current-2, current-1, current, current+1, current+2
const pageSlots = $derived([-2, -1, 0, 1, 2].map((delta) => currentPage + delta));

function isSlotVisible(page: number): boolean {
  return page >= 1 && page <= totalPages;
}

function goToPage(page: number) {
  direction = page > currentPage ? 1 : -1;
  onPageChange((page - 1) * limit);
}

// Build items array for the Select dropdown
const pageItems = $derived(
  Array.from({ length: totalPages }, (_, i) => ({
    value: String(i + 1),
    label: String(i + 1),
  }))
);

const selectValue = $derived(String(currentPage));
</script>

{#if variant === "simple"}
  {@const simpleTotalPages = Math.max(1, Math.ceil(totalCount / perPage))}
  {@const simpleStart = (simplePage - 1) * perPage + 1}
  {@const simpleEnd = Math.min(simplePage * perPage, totalCount)}
  <div class="mt-4 flex items-center justify-between text-sm">
    <span class="text-muted-foreground">
      Showing {simpleStart}&ndash;{simpleEnd} of {totalCount}
    </span>
    <div class="flex items-center gap-1">
      <button
        onclick={() => onPageChange(simplePage - 1)}
        disabled={simplePage <= 1}
        class="inline-flex items-center justify-center size-8 rounded-md bg-muted text-foreground
               hover:bg-accent transition-colors disabled:opacity-50 cursor-pointer disabled:cursor-default"
        aria-label="Previous page"
      >
        <ChevronLeft size={16} />
      </button>
      <span class="text-muted-foreground tabular-nums px-2">
        {simplePage} / {simpleTotalPages}
      </span>
      <button
        onclick={() => onPageChange(simplePage + 1)}
        disabled={simplePage >= simpleTotalPages}
        class="inline-flex items-center justify-center size-8 rounded-md bg-muted text-foreground
               hover:bg-accent transition-colors disabled:opacity-50 cursor-pointer disabled:cursor-default"
        aria-label="Next page"
      >
        <ChevronRight size={16} />
      </button>
    </div>
  </div>
{:else if totalCount > 0 && totalPages > 1}
    <div class="flex items-start text-xs mt-2 pl-2">
        <!-- Left zone: result count -->
        <div class="flex-1">
            <span class="text-muted-foreground select-none hidden md:inline">
                Showing {formatNumber(start)}&ndash;{formatNumber(end)} of {formatNumber(
                    totalCount,
                )} courses
            </span>
            <span class="text-muted-foreground select-none tabular-nums md:hidden">
                {formatNumber(start)}&ndash;{formatNumber(end)} / {formatNumber(totalCount)}
            </span>
        </div>

        <!-- Center zone: page buttons -->
        <div class="flex items-center gap-1">
            {#key currentPage}
                {#each pageSlots as page, i (i)}
                    {#if i === 2}
                        <!-- Center slot: current page with dropdown trigger -->
                        <Select.Root
                            type="single"
                            value={selectValue}
                            onValueChange={(v) => {
                                if (v) goToPage(Number(v));
                            }}
                            items={pageItems}
                        >
                            <Select.Trigger
                                class="inline-flex items-center justify-center gap-1 w-auto min-w-9 h-9 px-2.5
                       rounded-md text-sm font-medium tabular-nums
                       border border-border bg-card text-foreground
                       hover:bg-muted/50 active:bg-muted transition-colors
                       cursor-pointer select-none outline-none
                       focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background
                       {loading ? 'animate-pulse' : ''}"
                                aria-label="Page {currentPage} of {totalPages}, click to select page"
                            >
                                <span use:slideIn={direction}
                                    >{currentPage}</span
                                >
                                <ChevronUp
                                    class="size-3 text-muted-foreground"
                                />
                            </Select.Trigger>
                            <Select.Portal>
                                <Select.Content
                                    class="border border-border bg-card shadow-md outline-hidden z-50
                         max-h-72 min-w-16 w-auto
                         select-none rounded-md p-1
                         data-[state=open]:animate-in data-[state=closed]:animate-out
                         data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0
                         data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95
                         data-[side=top]:slide-in-from-bottom-2
                         data-[side=bottom]:slide-in-from-top-2"
                                    side="top"
                                    sideOffset={6}
                                >
                                    <Select.ScrollUpButton
                                        class="flex w-full items-center justify-center py-0.5"
                                    >
                                        <ChevronUp
                                            class="size-3.5 text-muted-foreground"
                                        />
                                    </Select.ScrollUpButton>
                                    <Select.Viewport class="p-0.5">
                                        {#each pageItems as item (item.value)}
                                            <Select.Item
                                                class="rounded-sm outline-hidden flex h-8 w-full select-none items-center
                               justify-center px-3 text-sm tabular-nums
                               data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground
                               data-[selected]:font-semibold"
                                                value={item.value}
                                                label={item.label}
                                            >
                                                {item.label}
                                            </Select.Item>
                                        {/each}
                                    </Select.Viewport>
                                    <Select.ScrollDownButton
                                        class="flex w-full items-center justify-center py-0.5"
                                    >
                                        <ChevronDown
                                            class="size-3.5 text-muted-foreground"
                                        />
                                    </Select.ScrollDownButton>
                                </Select.Content>
                            </Select.Portal>
                        </Select.Root>
                    {:else}
                        <!-- Side slot: navigable page button or invisible placeholder -->
                        <button
                            class="inline-flex items-center justify-center w-9 h-9
                     rounded-md text-sm tabular-nums
                     text-muted-foreground
                     hover:bg-muted/50 hover:text-foreground active:bg-muted transition-colors
                     cursor-pointer select-none
                     focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background
                     {!isSlotVisible(page)
                                ? 'invisible'
                                : loading
                                  ? 'opacity-40'
                                  : ''}
                     {!isSlotVisible(page) || loading
                                ? 'pointer-events-none'
                                : ''}"
                            onclick={() => goToPage(page)}
                            aria-label="Go to page {page}"
                            aria-hidden={!isSlotVisible(page)}
                            tabindex={isSlotVisible(page) ? 0 : -1}
                            disabled={!isSlotVisible(page) || loading}
                            use:slideIn={direction}
                        >
                            {page}
                        </button>
                    {/if}
                {/each}
            {/key}
        </div>

        <!-- Right zone: spacer for centering -->
        <div class="flex-1"></div>
    </div>
{:else if totalCount > 0}
    <!-- Single page: just show the count, no pagination controls -->
    <div class="flex items-start text-xs mt-2 pl-2">
        <span class="text-muted-foreground select-none hidden md:inline">
            Showing {formatNumber(start)}&ndash;{formatNumber(end)} of {formatNumber(
                totalCount,
            )} courses
        </span>
        <span class="text-muted-foreground select-none tabular-nums md:hidden">
            {formatNumber(start)}&ndash;{formatNumber(end)} / {formatNumber(totalCount)}
        </span>
    </div>
{/if}
