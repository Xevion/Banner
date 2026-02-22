import { BannerApiClient } from "$lib/api";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ fetch }) => {
  const client = new BannerApiClient(undefined, fetch);
  const result = await client.getAdminTerms();
  if (result.isErr) {
    return { terms: [], error: result.error.message };
  }
  return { terms: result.value.terms, error: null };
};
