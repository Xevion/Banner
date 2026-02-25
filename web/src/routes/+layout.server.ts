import type { User } from "$lib/bindings";
import type { LayoutServerLoad } from "./$types";

export const load: LayoutServerLoad = async ({ fetch }) => {
  try {
    const response = await fetch("/api/auth/me");
    if (response.ok) {
      const user = (await response.json()) as User;
      return { user };
    }
  } catch {
    // Network error or backend not ready -- treat as unauthenticated
  }

  return { user: null as User | null };
};
