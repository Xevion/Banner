import { goto } from "$app/navigation";
import type { User } from "$lib/bindings";

type AuthState =
  | { mode: "loading" }
  | { mode: "authenticated"; user: User }
  | { mode: "unauthenticated" };

class AuthStore {
  state = $state<AuthState>({ mode: "loading" });

  get user(): User | null {
    return this.state.mode === "authenticated" ? this.state.user : null;
  }

  get isAdmin(): boolean {
    return this.user?.isAdmin ?? false;
  }

  get isLoading(): boolean {
    return this.state.mode === "loading";
  }

  get isAuthenticated(): boolean {
    return this.state.mode === "authenticated";
  }

  /**
   * Attempt to load the current user session from the backend.
   * Only transitions to "unauthenticated" on a definitive 401/403.
   * Retries indefinitely on transient failures (network errors, 5xx)
   * so that a slow backend startup doesn't kick the user to login.
   */
  async init() {
    const MAX_DELAY_MS = 7_000;
    let delayMs = 500;

    for (;;) {
      try {
        const response = await fetch("/api/auth/me");

        if (response.ok) {
          const user = (await response.json()) as User;
          this.state = { mode: "authenticated", user };
          return;
        }

        // Definitive rejection — no session or not authorized
        if (response.status === 401 || response.status === 403) {
          this.state = { mode: "unauthenticated" };
          return;
        }

        // Server error (5xx) or unexpected status — retry
      } catch {
        // Network error (backend not up yet) — retry
      }

      await new Promise((r) => setTimeout(r, delayMs));
      delayMs = Math.min(delayMs * 2, MAX_DELAY_MS);
    }
  }

  /** Idempotently mark the session as lost. Called by apiFetch on 401. */
  handleUnauthorized() {
    if (this.state.mode !== "unauthenticated") {
      this.state = { mode: "unauthenticated" };
    }
  }

  login() {
    void goto("/api/auth/login");
  }

  async logout() {
    try {
      await fetch("/api/auth/logout", { method: "POST" });
    } finally {
      this.state = { mode: "unauthenticated" };
      void goto("/");
    }
  }
}

export const authStore = new AuthStore();
