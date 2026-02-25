<script lang="ts">
import { themeStore } from "$lib/stores/theme.svelte";
import type { Snippet } from "svelte";

let { children, theme }: { children: Snippet; theme: "light" | "dark" } = $props();

$effect(() => {
  themeStore.isDark = theme === "dark";
  document.documentElement.classList.toggle("dark", theme === "dark");
});
</script>

{@render children()}
