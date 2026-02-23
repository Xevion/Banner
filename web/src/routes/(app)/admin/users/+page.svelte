<script lang="ts">
import { client } from "$lib/api";
import type { User } from "$lib/bindings";
import { Shield, ShieldOff } from "@lucide/svelte";
import type { PageProps } from "./$types";

let { data }: PageProps = $props();
let users = $state<User[]>(data.users);
let error = $state<string | null>(data.error);
let updating = $state<string | null>(null);

async function toggleAdmin(user: User) {
  updating = user.discordId;
  const result = await client.setUserAdmin(user.discordId, !user.isAdmin);
  if (result.isErr) {
    error = result.error.message;
  } else {
    const updated = result.value;
    users = users.map((u) => (u.discordId === updated.discordId ? updated : u));
  }
  updating = null;
}
</script>

<svelte:head>
  <title>Users | Banner</title>
</svelte:head>

<h1 class="mb-4 text-lg font-semibold text-foreground">Users</h1>

{#if error}
  <p class="text-destructive mb-4">{error}</p>
{/if}

{#if users.length === 0 && !error}
  <p class="text-muted-foreground">Loading...</p>
{:else}
  <div class="bg-card border-border overflow-hidden rounded-lg border">
    <table class="w-full text-sm">
      <thead>
        <tr class="border-border border-b">
          <th class="px-4 py-3 text-left font-medium">Username</th>
          <th class="px-4 py-3 text-left font-medium">Discord ID</th>
          <th class="px-4 py-3 text-left font-medium">Admin</th>
          <th class="px-4 py-3 text-left font-medium">Actions</th>
        </tr>
      </thead>
      <tbody>
        {#each users as user (user.discordId)}
          <tr class="border-border border-b last:border-b-0">
            <td class="flex items-center gap-2 px-4 py-3">
              {#if user.discordAvatarHash}
                <img
                  src="https://cdn.discordapp.com/avatars/{user.discordId}/{user.discordAvatarHash}.png?size=32"
                  alt=""
                  class="h-6 w-6 rounded-full"
                />
              {/if}
              {user.discordUsername}
            </td>
            <td class="text-muted-foreground px-4 py-3 font-mono text-xs"
              >{user.discordId}</td
            >
            <td class="px-4 py-3">
              {#if user.isAdmin}
                <span
                  class="rounded-full bg-blue-100 px-2 py-0.5 text-xs font-medium text-blue-800 dark:bg-blue-900 dark:text-blue-200"
                  >Admin</span
                >
              {:else}
                <span class="text-muted-foreground text-xs">User</span>
              {/if}
            </td>
            <td class="px-4 py-3">
              <button
                onclick={() => toggleAdmin(user)}
                disabled={updating === user.discordId}
                class="hover:bg-accent inline-flex items-center gap-1 rounded px-2 py-1 text-xs transition-colors disabled:opacity-50"
              >
                {#if user.isAdmin}
                  <ShieldOff size={14} />
                  Remove Admin
                {:else}
                  <Shield size={14} />
                  Make Admin
                {/if}
              </button>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{/if}
