<script lang="ts">
import { goto } from "$app/navigation";
import { page } from "$app/state";
import { authStore } from "$lib/auth.svelte";
import BottomSheet from "$lib/components/BottomSheet.svelte";
import ErrorBoundaryFallback from "$lib/components/ErrorBoundaryFallback.svelte";
import {
  Activity,
  Calendar,
  GraduationCap,
  LayoutDashboard,
  LogOut,
  MoreHorizontal,
  Settings,
  User,
  Users,
} from "@lucide/svelte";
import { tick, type Snippet } from "svelte";

let { children }: { children: Snippet } = $props();

let moreSheetOpen = $state(false);

// Track boundary reset function so navigation can auto-clear errors
let boundaryReset = $state<(() => void) | null>(null);
let errorPathname = $state<string | null>(null);

function onBoundaryError(e: unknown, reset: () => void) {
  console.error("[page boundary]", e);
  boundaryReset = reset;
  errorPathname = page.url.pathname;
}

// Auto-reset the boundary only when the user navigates away from the errored page
$effect(() => {
  const currentPath = page.url.pathname;

  if (boundaryReset && errorPathname && currentPath !== errorPathname) {
    const reset = boundaryReset;
    boundaryReset = null;
    errorPathname = null;
    void tick().then(() => reset());
  }
});

$effect(() => {
  if (authStore.state.mode === "unauthenticated") {
    void goto("/login");
  }
});

const userItems = [
  { href: "/profile", label: "Profile", icon: User },
  { href: "/settings", label: "Settings", icon: Settings },
];

const adminItems = [
  { href: "/admin", label: "Dashboard", icon: LayoutDashboard },
  { href: "/admin/scraper", label: "Scraper", icon: Activity },
  { href: "/admin/terms", label: "Terms", icon: Calendar },
  { href: "/admin/users", label: "Users", icon: Users },
  { href: "/admin/instructors", label: "Instructors", icon: GraduationCap },
];

function isActive(href: string): boolean {
  if (href === "/admin") return page.url.pathname === "/admin";
  return page.url.pathname.startsWith(href);
}

// Bottom tab bar definitions
const adminTabs = [
  { href: "/profile", icon: User, label: "Profile" },
  { href: "/admin", icon: LayoutDashboard, label: "Dashboard" },
  { href: "/admin/scraper", icon: Activity, label: "Scraper" },
] as const;

const nonAdminTabs = [
  { href: "/profile", icon: User, label: "Profile" },
  { href: "/settings", icon: Settings, label: "Settings" },
] as const;

// "More" sheet items (admin only, items not in the tab bar)
const moreSheetItems = [
  { href: "/settings", icon: Settings, label: "Settings" },
  { href: "/admin/terms", icon: Calendar, label: "Terms" },
  { href: "/admin/users", icon: Users, label: "Users" },
  { href: "/admin/instructors", icon: GraduationCap, label: "Instructors" },
] as const;
</script>

{#if authStore.isLoading}
  <div class="flex flex-col items-center px-5 pb-5 pt-20">
    <div class="w-full max-w-6xl">
      <p class="text-muted-foreground py-12 text-center text-sm">Loading...</p>
    </div>
  </div>
{:else if !authStore.isAuthenticated}
  <div class="flex flex-col items-center px-5 pb-5 pt-20">
    <div class="w-full max-w-6xl">
      <p class="text-muted-foreground py-12 text-center text-sm">Redirecting to login...</p>
    </div>
  </div>
{:else}
  <div class="flex flex-col items-center px-3 md:px-5 pb-20 md:pb-5 pt-20">
    <div class="w-full max-w-6xl flex gap-8">
      <!-- Inline sidebar -->
      <aside class="hidden md:block w-48 shrink-0 pt-1">
        {#if authStore.user}
          <div class="mb-4 px-2">
            <p class="text-sm font-medium text-foreground">{authStore.user.discordUsername}</p>
          </div>
        {/if}

        <nav class="flex flex-col gap-0.5">
          <span class="px-2 text-[11px] font-medium uppercase tracking-wider text-muted-foreground/60 mb-0.5">User</span>
          {#each userItems as item (item.href)}
            <a
              href={item.href}
              class="flex items-center gap-2 rounded-md px-2 py-1.5 text-sm no-underline transition-colors
                {isActive(item.href)
                  ? 'text-foreground bg-muted font-medium'
                  : 'text-muted-foreground hover:text-foreground hover:bg-muted/50'}"
            >
              <item.icon size={15} strokeWidth={2} />
              {item.label}
            </a>
          {/each}

          {#if authStore.isAdmin}
            <div class="my-2 mx-2 border-t border-border"></div>
            <span class="px-2 text-[11px] font-medium uppercase tracking-wider text-muted-foreground/60 mb-0.5">Admin</span>
            {#each adminItems as item (item.href)}
              <a
                href={item.href}
                class="flex items-center gap-2 rounded-md px-2 py-1.5 text-sm no-underline transition-colors
                  {isActive(item.href)
                    ? 'text-foreground bg-muted font-medium'
                    : 'text-muted-foreground hover:text-foreground hover:bg-muted/50'}"
              >
                <item.icon size={15} strokeWidth={2} />
                {item.label}
              </a>
            {/each}
          {/if}

          <div class="my-2 mx-2 border-t border-border"></div>
          <button
            onclick={() => authStore.logout()}
            class="flex items-center gap-2 rounded-md px-2 py-1.5 text-sm cursor-pointer
              bg-transparent border-none text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors"
          >
            <LogOut size={15} strokeWidth={2} />
            Sign Out
          </button>
        </nav>
      </aside>

      <!-- Content -->
      <main class="flex-1 min-w-0" style="view-transition-name: app-content">
        <svelte:boundary onerror={onBoundaryError}>
            {@render children()}

          {#snippet failed(error, reset)}
            <ErrorBoundaryFallback title="Page error" {error} {reset} />
          {/snippet}
        </svelte:boundary>
      </main>
    </div>
  </div>

  <!-- Mobile bottom tab bar -->
  <nav class="fixed bottom-0 inset-x-0 z-30 md:hidden bg-background/95 backdrop-blur-md border-t border-border pb-[env(safe-area-inset-bottom)]">
    <div class="flex">
      {#each authStore.isAdmin ? adminTabs : nonAdminTabs as tab (tab.href)}
        <a
          href={tab.href}
          class="flex flex-col items-center justify-center gap-0.5 flex-1 py-2 min-h-[56px] no-underline
            {isActive(tab.href) ? 'text-foreground' : 'text-muted-foreground'}"
        >
          <tab.icon size={20} strokeWidth={1.75} />
          <span class="text-[10px] font-medium">{tab.label}</span>
        </a>
      {/each}
      {#if authStore.isAdmin}
        <button
          onclick={() => (moreSheetOpen = true)}
          class="flex flex-col items-center justify-center gap-0.5 flex-1 py-2 min-h-[56px]
            bg-transparent border-none cursor-pointer text-muted-foreground"
        >
          <MoreHorizontal size={20} strokeWidth={1.75} />
          <span class="text-[10px] font-medium">More</span>
        </button>
      {:else}
        <button
          onclick={() => authStore.logout()}
          class="flex flex-col items-center justify-center gap-0.5 flex-1 py-2 min-h-[56px]
            bg-transparent border-none cursor-pointer text-muted-foreground"
        >
          <LogOut size={20} strokeWidth={1.75} />
          <span class="text-[10px] font-medium">Sign Out</span>
        </button>
      {/if}
    </div>
  </nav>

  <!-- Admin "More" bottom sheet -->
  {#if authStore.isAdmin}
    <BottomSheet bind:open={moreSheetOpen} maxHeight="60vh" label="More options">
      <nav class="flex flex-col gap-0.5 px-4 pb-4">
        {#each moreSheetItems as item (item.href)}
          <a
            href={item.href}
            onclick={() => (moreSheetOpen = false)}
            class="flex items-center gap-2 rounded-md px-2 py-1.5 text-sm no-underline transition-colors
              {isActive(item.href)
                ? 'text-foreground bg-muted font-medium'
                : 'text-muted-foreground hover:text-foreground hover:bg-muted/50'}"
          >
            <item.icon size={15} strokeWidth={2} />
            {item.label}
          </a>
        {/each}
        <div class="my-2 mx-2 border-t border-border"></div>
        <button
          onclick={() => { moreSheetOpen = false; void authStore.logout(); }}
          class="flex items-center gap-2 rounded-md px-2 py-1.5 text-sm cursor-pointer
            bg-transparent border-none text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors"
        >
          <LogOut size={15} strokeWidth={2} />
          Sign Out
        </button>
      </nav>
    </BottomSheet>
  {/if}
{/if}
