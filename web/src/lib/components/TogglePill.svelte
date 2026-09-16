<script lang="ts">
let {
  active,
  onclick,
  label,
  title,
  ariaLabel,
  variant = "rounded-full",
  flex = false,
}: {
  active: boolean;
  onclick: () => void;
  label: string;
  title?: string;
  ariaLabel?: string;
  variant?: "rounded-full" | "rounded-md";
  flex?: boolean;
} = $props();
</script>

<button
  type="button"
  aria-pressed={active}
  aria-label={ariaLabel}
  class="inline-flex items-center justify-center {variant} px-2 py-0.5 text-xs font-medium transition-colors cursor-pointer select-none
         {active
    ? 'bg-primary text-primary-foreground'
    : 'bg-muted text-muted-foreground hover:bg-muted/80'}
         {variant === 'rounded-md' ? 'py-1 px-2' : ''}
         {flex ? 'flex-1' : ''}"
  {onclick}
  {title}
>
  {label}
</button>
