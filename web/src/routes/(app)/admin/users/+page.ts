import { BannerApiClient } from "$lib/api";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ fetch }) => {
  const client = new BannerApiClient(undefined, fetch);
  const result = await client.getAdminUsers();
  if (result.isErr) {
    return { users: [], error: result.error.message };
  }
  return { users: result.value, error: null };
};
