<script lang="ts">
interface Variant {
  code: string;
  label: string;
}

let {
  label,
  codes,
  variants = [],
  selected = $bindable<string[]>([]),
}: {
  label: string;
  codes: string[];
  variants?: Variant[] | undefined;
  selected: string[];
} = $props();

let hasVariants = $derived(variants.length > 0);
let hasSomeSelected = $derived(codes.some((code) => selected.includes(code)));
let hasAllSelected = $derived(
  hasVariants
    ? variants.every((v) => selected.includes(v.code))
    : codes.every((code) => selected.includes(code))
);

function toggleAll() {
  if (hasAllSelected) {
    selected = selected.filter((c) => !codes.includes(c));
  } else if (hasVariants) {
    const variantCodes = variants.map((v) => v.code);
    selected = [...selected.filter((c) => !codes.includes(c)), ...variantCodes];
  } else {
    selected = [...selected.filter((c) => !codes.includes(c)), ...codes];
  }
}

function toggleVariant(code: string) {
  if (selected.includes(code)) {
    selected = selected.filter((c) => c !== code);
  } else {
    selected = [...selected, code];
  }
}
</script>

{#if hasVariants}
  <!-- Compound button with parent + sub-variant buttons -->
  <button
    type="button"
    class="flex w-full items-center gap-2 rounded-lg border px-2.5 py-1 transition-colors cursor-pointer select-none
           {hasSomeSelected
      ? 'border-primary/30 bg-primary/10 text-primary'
      : 'border-border bg-muted text-muted-foreground hover:bg-muted/80'}"
    onclick={toggleAll}
    aria-pressed={hasAllSelected}
  >
    <span class="text-sm font-medium">{label}</span>
    <span class="ml-auto flex items-center gap-1 whitespace-nowrap">
      {#each variants as variant (variant.code)}
        {@const isSelected = selected.includes(variant.code)}
        <span
          role="button"
          tabindex="0"
          class="rounded-md border px-2 py-1 text-xs font-medium transition-colors cursor-pointer
                 {isSelected
            ? 'border-primary bg-primary text-primary-foreground'
            : hasSomeSelected
              ? 'border-primary/40 bg-primary/20 text-primary hover:bg-primary/30'
              : 'border-border bg-background text-muted-foreground hover:bg-muted'}"
          onclick={(e: MouseEvent) => {
            e.stopPropagation();
            toggleVariant(variant.code);
          }}
          onkeydown={(e: KeyboardEvent) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              e.stopPropagation();
              toggleVariant(variant.code);
            }
          }}
          aria-pressed={isSelected}
        >
          {variant.label}
        </span>
      {/each}
    </span>
  </button>
{:else}
  <!-- Simple toggle button (no variants) -->
  <button
    type="button"
    class="flex w-full items-center rounded-lg border px-3 py-1.5 text-sm font-medium transition-colors cursor-pointer select-none
           {hasSomeSelected
      ? 'border-primary/30 bg-primary text-primary-foreground'
      : 'border-border bg-muted text-muted-foreground hover:bg-muted/80'}"
    onclick={toggleAll}
    aria-pressed={hasSomeSelected}
  >
    {label}
  </button>
{/if}
