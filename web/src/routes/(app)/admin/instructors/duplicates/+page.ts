import { BannerApiClient } from "$lib/api";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ fetch }) => {
  const client = new BannerApiClient(undefined, fetch);
  const result = await client.getInstructorDuplicates();

  return {
    pairs: result.isOk ? result.value.pairs : [],
    error: result.isErr ? result.error.message : null,
  };
};
