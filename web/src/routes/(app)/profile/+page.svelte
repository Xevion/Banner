<script lang="ts">
import { authStore } from "$lib/auth.svelte";
</script>

<svelte:head>
  <title>Profile | Banner</title>
</svelte:head>

<h1 class="mb-4 text-lg font-semibold text-foreground">Profile</h1>

{#if authStore.user}
  <div class="bg-card border-border rounded-lg border p-4">
    <div class="flex flex-col gap-3">
      <div>
        <p class="text-muted-foreground text-sm">Username</p>
        <p class="font-medium">{authStore.user.discordUsername}</p>
      </div>
      <div>
        <p class="text-muted-foreground text-sm">Discord ID</p>
        <p class="font-medium font-mono text-sm">{authStore.user.discordId}</p>
      </div>
      <div>
        <p class="text-muted-foreground text-sm">Role</p>
        <p class="font-medium">{authStore.isAdmin ? "Admin" : "User"}</p>
      </div>
    </div>
  </div>
{/if}
