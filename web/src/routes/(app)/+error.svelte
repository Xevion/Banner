<script lang="ts">
import { page } from "$app/state";
import { authStore } from "$lib/auth.svelte";
import { House, LogOut } from "@lucide/svelte";

const status = $derived(page.status);

const messages: Record<number, string> = {
  400: "Bad request",
  401: "Unauthorized",
  403: "You don't have permission to view this page",
  404: "Page not found",
  405: "Method not allowed",
  408: "Request timeout",
  429: "Too many requests",
  500: "Something went wrong",
  502: "Service temporarily unavailable",
  503: "Service temporarily unavailable",
  504: "Gateway timeout",
};

const message = $derived(messages[status] ?? "An error occurred");
const isServerError = $derived(status >= 500);
const isAuthError = $derived(status === 401 || status === 403);
</script>

<svelte:head>
  <title>{status} - {message} | Banner</title>
</svelte:head>

<div class="flex flex-1 items-center justify-center px-4">
  <div class="max-w-md text-center">
    <h1 class="text-8xl font-bold tracking-tight text-muted-foreground/50">{status}</h1>
    <p class="mt-4 text-xl text-muted-foreground">{message}</p>

    {#if isServerError}
      <p class="mt-2 text-sm text-muted-foreground/60">This may be temporary. Try again in a moment.</p>
    {/if}

    <div class="mt-8 flex items-center justify-center gap-3">
      {#if isAuthError}
        <button
          onclick={() => authStore.logout()}
          class="inline-flex items-center gap-2 rounded-lg border border-border bg-card px-4 py-2.5 text-sm font-medium text-foreground shadow-sm transition-colors hover:bg-muted cursor-pointer"
        >
          <LogOut size={16} strokeWidth={2} />
          Sign out
        </button>
      {/if}
      <a
        href="/"
        class="inline-flex items-center gap-2 rounded-lg border border-border bg-card px-4 py-2.5 text-sm font-medium text-foreground shadow-sm transition-colors hover:bg-muted"
      >
        <House size={16} strokeWidth={2} />
        Return home
      </a>
    </div>
  </div>
</div>
