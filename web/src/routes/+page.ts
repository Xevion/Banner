import { BannerApiClient } from "$lib/api";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ url, fetch }) => {
  const client = new BannerApiClient(undefined, fetch);
  const urlTerm = url.searchParams.get("term");
  // Backend defaults to latest term if not specified
  const result = await client.getSearchOptions(urlTerm ?? undefined);
  if (result.isErr) {
    console.error("Failed to load search options:", result.error);
    return { searchOptions: null, url };
  }
  return { searchOptions: result.value, url };
};
