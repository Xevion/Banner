import { beforeNavigate, onNavigate } from "$app/navigation";
import { browser } from "$app/environment";

export type NavDirection = "left" | "right" | "fade";
export type NavAxis = "horizontal" | "vertical";

/**
 * Path used for navbar label expansion. Deferred during view transitions so
 * CSS transitions run on visible DOM instead of snapping while hidden.
 * The pill animation (JS/RAF-driven) uses page.url.pathname directly.
 */
class NavbarState {
  path = $state("/");
}

export const navbar = new NavbarState();

/** Sidebar nav order — indexes determine slide direction for same-depth siblings */
const SIDEBAR_NAV_ORDER = [
  "/profile",
  "/settings",
  "/admin",
  "/admin/jobs",
  "/admin/audit",
  "/admin/users",
];

const APP_PREFIXES = ["/profile", "/settings", "/admin"];

function getDepth(path: string): number {
  return path.replace(/\/$/, "").split("/").filter(Boolean).length;
}

function getSidebarIndex(path: string): number {
  return SIDEBAR_NAV_ORDER.indexOf(path);
}

function computeDirection(from: string, to: string): NavDirection {
  const fromDepth = getDepth(from);
  const toDepth = getDepth(to);

  if (toDepth > fromDepth) return "right";
  if (toDepth < fromDepth) return "left";

  // Same depth — use sidebar ordering if both are sidebar routes
  const fromIdx = getSidebarIndex(from);
  const toIdx = getSidebarIndex(to);
  if (fromIdx >= 0 && toIdx >= 0) {
    return toIdx > fromIdx ? "right" : "left";
  }

  return "fade";
}

function computeAxis(from: string, to: string): NavAxis {
  const fromIsApp = APP_PREFIXES.some((p) => from.startsWith(p));
  const toIsApp = APP_PREFIXES.some((p) => to.startsWith(p));
  return fromIsApp && toIsApp ? "vertical" : "horizontal";
}

/** Call once from root layout to start tracking navigation direction */
export function initNavigation() {
  if (browser) {
    navbar.path = window.location.pathname;
  }

  beforeNavigate(({ from, to }) => {
    if (!from?.url || !to?.url) return;
    const fromPath = from.url.pathname;
    const toPath = to.url.pathname;
    document.documentElement.dataset.navDirection = computeDirection(fromPath, toPath);
    document.documentElement.dataset.navAxis = computeAxis(fromPath, toPath);
  });

  onNavigate((navigation) => {
    // Skip document-level view transitions for same-page navigations (e.g.
    // query param updates from filter changes). Document transitions apply
    // visibility:hidden to the entire page, blocking all pointer interaction.
    const fromPath = navigation.from?.url.pathname;
    const toPath = navigation.to?.url.pathname;
    const isPageChange = fromPath !== toPath;

    if (!document.startViewTransition) {
      void navigation.complete.then(() => {
        navbar.path = window.location.pathname;
      });
      return;
    }

    // Same-page navigations (e.g. search filter/pagination changes): use a scoped
    // view transition on the results table so popovers and other UI aren't affected.
    if (!isPageChange) {
      const tableEl = document.querySelector("[data-search-results]");
      if (tableEl && "startViewTransition" in tableEl) {
        interface ScopedVT {
          startViewTransition: (cb: () => Promise<void>) => void;
        }
        return new Promise((resolve) => {
          (tableEl as unknown as ScopedVT).startViewTransition(async () => {
            resolve();
            await navigation.complete;
          });
        });
      }
      // No table element (different page or not mounted yet) — skip transition
      void navigation.complete.then(() => {
        navbar.path = window.location.pathname;
      });
      return;
    }

    return new Promise((resolve) => {
      // Suppress non-navigation view-transition-names (e.g. search-results)
      // so they don't create independent transition groups during page nav.
      document.documentElement.classList.add("nav-transitioning");

      const vt = document.startViewTransition(async () => {
        resolve();
        await navigation.complete;
      });

      // Update navbar path only after the view transition finishes and the
      // real DOM is visible again, so CSS transitions can actually run.
      void vt.finished.finally(() => {
        document.documentElement.classList.remove("nav-transitioning");
        navbar.path = window.location.pathname;
      });
    });
  });
}
