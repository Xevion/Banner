import { BannerApiClient } from "$lib/api";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ fetch }) => {
  const client = new BannerApiClient(undefined, fetch);

  const result = await client.getSearchOptions();

  return {
    searchOptions: result.isOk ? result.value : null,
  };
};
