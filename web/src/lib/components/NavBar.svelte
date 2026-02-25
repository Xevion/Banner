<script lang="ts">
import { page } from "$app/state";
import { authStore } from "$lib/auth.svelte";
import { navbar } from "$lib/stores/navigation.svelte";
import { Clock, Search, User } from "@lucide/svelte";
import ThemeToggle from "./ThemeToggle.svelte";

const staticTabs = [
  { href: "/", label: "Search", icon: Search },
  { href: "/timeline", label: "Timeline", icon: Clock },
] as const;

const APP_PREFIXES = ["/profile", "/settings", "/admin"];

let profileTab = $derived(
  authStore.isLoading
    ? { href: "/login" as const, label: null, icon: User }
    : {
        href: authStore.isAuthenticated ? ("/profile" as const) : ("/login" as const),
        label: authStore.isAuthenticated ? "Account" : "Login",
        icon: User,
      }
);

function isActive(tabHref: string): boolean {
  if (tabHref === "/") return page.url.pathname === "/";
  if (tabHref === "/profile") {
    return APP_PREFIXES.some((p) => page.url.pathname.startsWith(p));
  }
  return page.url.pathname.startsWith(tabHref);
}

/** Label expansion check using a deferred path that updates only after
 *  view transitions finish, so CSS transitions run on visible DOM. */
function isLabelExpanded(tabHref: string): boolean {
  if (tabHref === "/") return navbar.path === "/";
  if (tabHref === "/profile") {
    return APP_PREFIXES.some((p) => navbar.path.startsWith(p));
  }
  return navbar.path.startsWith(tabHref);
}

// DOM refs
let tabRefs: HTMLAnchorElement[] = $state([]);
let containerRef: HTMLDivElement | undefined = $state();
let pillRef: HTMLDivElement | undefined = $state();

// Pill animation state -- driven by JS, not CSS transitions
let targetLeft = 0;
let targetWidth = 0;
let currentLeft = 0;
let currentWidth = 0;
let animationId: number | null = null;
let mounted = $state(false);

const ANIMATION_DURATION = 300;
const EASING = cubicOut;

function cubicOut(t: number): number {
  const f = t - 1;
  return f * f * f + 1;
}

function allTabs() {
  return [...staticTabs.map((t) => t.href), profileTab.href];
}

function activeIndex(): number {
  return allTabs().findIndex((href) => isActive(href));
}

function measureActiveTab(): { left: number; width: number } | null {
  const idx = activeIndex();
  if (idx < 0 || !tabRefs[idx] || !containerRef) return null;
  const containerRect = containerRef.getBoundingClientRect();
  const tabRect = tabRefs[idx].getBoundingClientRect();
  return {
    left: tabRect.left - containerRect.left,
    width: tabRect.width,
  };
}

function applyPill(left: number, width: number) {
  if (!pillRef) return;
  pillRef.style.transform = `translateX(${left}px)`;
  pillRef.style.width = `${width}px`;
  currentLeft = left;
  currentWidth = width;
}

function animatePill(fromLeft: number, fromWidth: number, toLeft: number, toWidth: number) {
  if (animationId !== null) {
    cancelAnimationFrame(animationId);
    animationId = null;
  }

  const startTime = performance.now();

  function tick(now: number) {
    const elapsed = now - startTime;
    const progress = Math.min(elapsed / ANIMATION_DURATION, 1);
    const eased = EASING(progress);

    const left = fromLeft + (toLeft - fromLeft) * eased;
    const width = fromWidth + (toWidth - fromWidth) * eased;
    applyPill(left, width);

    if (progress < 1) {
      animationId = requestAnimationFrame(tick);
    } else {
      animationId = null;
    }
  }

  animationId = requestAnimationFrame(tick);
}

function updateTarget() {
  const measured = measureActiveTab();
  if (!measured) return;

  targetLeft = measured.left;
  targetWidth = measured.width;

  if (!mounted) {
    // First render -- snap immediately, no animation
    applyPill(targetLeft, targetWidth);
    mounted = true;
    return;
  }

  // Always (re)start animation from current position -- handles both fresh
  // navigations and rapid route changes that interrupt a running animation
  if (animationId !== null) {
    cancelAnimationFrame(animationId);
    animationId = null;
  }
  animatePill(currentLeft, currentWidth, targetLeft, targetWidth);
}

function updateTargetFromResize() {
  const measured = measureActiveTab();
  if (!measured) return;

  const newLeft = measured.left;
  const newWidth = measured.width;

  // If nothing changed, skip
  if (newLeft === targetLeft && newWidth === targetWidth) return;

  targetLeft = newLeft;
  targetWidth = newWidth;

  if (animationId !== null) {
    // Animation in progress -- retarget it smoothly by starting a new
    // animation from the current interpolated position to the new target
    cancelAnimationFrame(animationId);
    animationId = null;
    animatePill(currentLeft, currentWidth, targetLeft, targetWidth);
  } else {
    // No animation running -- snap (this handles window resize, etc.)
    applyPill(targetLeft, targetWidth);
  }
}

// Start animation when route changes
$effect(() => {
  void page.url.pathname;
  void profileTab.href;

  requestAnimationFrame(() => {
    updateTarget();
  });
});

// Track the active tab's size during label transitions and window resizes
$effect(() => {
  if (!containerRef) return;
  const observer = new ResizeObserver(() => {
    updateTargetFromResize();
  });
  observer.observe(containerRef);
  for (const ref of tabRefs) {
    if (ref) observer.observe(ref);
  }
  return () => observer.disconnect();
});
</script>

<nav class="w-full flex justify-center pt-5 px-3 sm:px-5">
  <div class="w-full max-w-6xl flex items-center justify-between">
    <!-- pointer-events-auto: root layout wraps nav in pointer-events-none overlay -->
    <div
      class="relative flex items-center gap-1 rounded-lg bg-muted p-1 pointer-events-auto"
      bind:this={containerRef}
    >
      <!-- Sliding pill -- animated via JS (RAF) to stay smooth even when
           heavy page transitions cause CSS transition skipping -->
      <div
        class="absolute top-1 bottom-1 left-0 rounded-md bg-background shadow-sm will-change-[transform,width]"
        bind:this={pillRef}
      ></div>

      {#each staticTabs as tab, i (tab.href)}
        <a
          href={tab.href}
          bind:this={tabRefs[i]}
          class="relative z-10 flex items-center gap-1.5 rounded-md px-2 sm:px-3 py-1.5 text-sm font-medium transition-colors no-underline select-none
            {isActive(tab.href) ? 'text-foreground' : 'text-muted-foreground hover:text-foreground'}"
        >
          <tab.icon size={15} strokeWidth={2} />
          <span
            class="grid overflow-hidden transition-[grid-template-columns,opacity] duration-300 ease-[cubic-bezier(0.4,0,0.2,1)]
              {isLabelExpanded(tab.href)
                ? 'grid-cols-[1fr] opacity-100'
                : 'grid-cols-[0fr] opacity-0 sm:grid-cols-[1fr] sm:opacity-100'}"
          >
            <span class="overflow-hidden whitespace-nowrap">{tab.label}</span>
          </span>
        </a>
      {/each}
      <a
        href={profileTab.href}
        bind:this={tabRefs[staticTabs.length]}
        class="relative z-10 flex items-center gap-1.5 rounded-md px-2 sm:px-3 py-1.5 text-sm font-medium transition-colors no-underline select-none
          {isActive(profileTab.href)
            ? 'text-foreground'
            : 'text-muted-foreground hover:text-foreground'}"
      >
        <User size={15} strokeWidth={2} />
        {#if profileTab.label}
          <span
            class="grid overflow-hidden transition-[grid-template-columns,opacity] duration-300 ease-[cubic-bezier(0.4,0,0.2,1)]
              {isLabelExpanded(profileTab.href)
                ? 'grid-cols-[1fr] opacity-100'
                : 'grid-cols-[0fr] opacity-0 sm:grid-cols-[1fr] sm:opacity-100'}"
          >
            <span class="overflow-hidden whitespace-nowrap">{profileTab.label}</span>
          </span>
        {/if}
      </a>
      <ThemeToggle />
    </div>
  </div>
</nav>
