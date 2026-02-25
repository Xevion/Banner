<script lang="ts">
import { authStore } from "$lib/auth.svelte";
import type { User } from "$lib/bindings";
import type { Snippet } from "svelte";

export type AuthMode = "unauthenticated" | "authenticated" | "admin" | "loading";

let { children, authMode }: { children: Snippet; authMode: AuthMode } = $props();

const mockUser: User = {
  discordId: "111222333444555666",
  discordUsername: "StoryUser",
  discordAvatarHash: null,
  isAdmin: false,
  createdAt: "2024-01-01T00:00:00Z",
  updatedAt: "2024-01-01T00:00:00Z",
};

const mockAdmin: User = {
  ...mockUser,
  discordUsername: "AdminUser",
  isAdmin: true,
};

$effect(() => {
  if (authMode === "authenticated") {
    authStore.setFromServer(mockUser);
  } else if (authMode === "admin") {
    authStore.setFromServer(mockAdmin);
  } else if (authMode === "unauthenticated") {
    authStore.setFromServer(null);
  }
  // 'loading' — leave as-is (store initializes to { mode: "loading" })
});
</script>

{@render children()}
