import { goto } from "$app/navigation";
import type { User } from "$lib/bindings";
import { telemetry } from "$lib/telemetry";

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
   * Initialize from server-provided user data (from +layout.server.ts).
   * Called once on hydration with the user from the server load.
   */
  setFromServer(user: User | null) {
    if (user) {
      this.state = { mode: "authenticated", user };
      telemetry.identify(user.discordId, {
        username: user.discordUsername,
        isAdmin: user.isAdmin,
      });
    } else {
      this.state = { mode: "unauthenticated" };
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
      telemetry.track({ name: "auth", properties: { action: "logout" } });
    } finally {
      this.state = { mode: "unauthenticated" };
      telemetry.reset();
      void goto("/");
    }
  }
}

export const authStore = new AuthStore();
