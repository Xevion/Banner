import { BannerApiClient } from "$lib/api";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ fetch }) => {
  const client = new BannerApiClient(undefined, fetch);
  const result = await client.getAdminStatus();
  if (result.isErr) {
    return { status: null, error: result.error.message };
  }
  return { status: result.value, error: null };
};
