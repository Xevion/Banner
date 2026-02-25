<script lang="ts">
import "overlayscrollbars/overlayscrollbars.css";
import "./layout.css";
import { authStore } from "$lib/auth.svelte";
import ErrorBoundaryFallback from "$lib/components/ErrorBoundaryFallback.svelte";
import NavBar from "$lib/components/NavBar.svelte";
import { useOverlayScrollbars } from "$lib/composables/useOverlayScrollbars.svelte";
import { initNavigation } from "$lib/stores/navigation.svelte";
import { themeStore } from "$lib/stores/theme.svelte";
import { Tooltip } from "bits-ui";
import { type Snippet, onMount } from "svelte";
import type { LayoutData } from "./$types";

let { children, data }: { children: Snippet; data: LayoutData } = $props();

// Initialize auth from server-provided data so SSR renders correct auth state.
// Intentionally captures initial value -- auth changes are handled client-side
// by the store (logout, 401 responses). Root layout server load only re-runs
// on full page loads, so data.user is stable within a session.
// svelte-ignore state_referenced_locally
authStore.setFromServer(data.user);

initNavigation();

useOverlayScrollbars(() => document.body, {
  scrollbars: {
    autoHide: "leave",
    autoHideDelay: 800,
  },
});

onMount(() => {
  themeStore.init();
});
</script>

<svelte:head>
  <title>Banner</title>
</svelte:head>

<Tooltip.Provider delayDuration={150} skipDelayDuration={50}>
  <div class="relative flex min-h-screen flex-col overflow-x-hidden">
    <!-- pointer-events-none so the navbar doesn't block canvas interactions;
         NavBar re-enables pointer-events on its own container. -->
    <div class="absolute inset-x-0 top-0 z-50 pointer-events-none" style="view-transition-name: navbar">
      <NavBar />
    </div>

    <svelte:boundary onerror={(e) => console.error("[root boundary]", e)}>
        {@render children()}

      {#snippet failed(error, reset)}
        <ErrorBoundaryFallback {error} {reset} />
      {/snippet}
    </svelte:boundary>
  </div>
</Tooltip.Provider>
