<script lang="ts">
let {
  value,
  label,
  style = "",
}: {
  value: string;
  label: string;
  style?: string;
} = $props();
</script>

<div class="text-center">
  <div class="text-xl font-semibold" {style}>{value}</div>
  <div class="text-xs text-muted-foreground mt-0.5">{label}</div>
</div>
