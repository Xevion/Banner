import { parseActionLogParams } from "$lib/action-log";
import { BannerApiClient } from "$lib/api";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ fetch, url }) => {
  const client = new BannerApiClient(undefined, fetch);
  const filters = parseActionLogParams(url.searchParams);
  const [result, users] = await Promise.all([
    client.getAdminActionLog(filters),
    client.getAdminUsers(),
  ]);

  return {
    filters,
    log: result.isOk ? result.value : null,
    error: result.isErr ? result.error.message : null,
    // Only admins can act, so the actor filter lists them alone.
    actors: users.isOk ? users.value.filter((user) => user.isAdmin) : [],
  };
};
