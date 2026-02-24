import { BannerApiClient } from "$lib/api";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ fetch }) => {
  const client = new BannerApiClient(undefined, fetch);
  const result = await client.getAdminBluebookLinks({ page: 1, perPage: 25 });

  return {
    links: result.isOk ? result.value : null,
    error: result.isErr ? result.error.message : null,
  };
};
