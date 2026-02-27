import { BannerApiClient } from "$lib/api";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ url, fetch }) => {
  const client = new BannerApiClient(undefined, fetch);

  const search = url.searchParams.get("search") ?? undefined;
  const subject = url.searchParams.get("subject") ?? undefined;
  const sort = url.searchParams.get("sort") ?? undefined;
  const page = url.searchParams.has("page") ? Number(url.searchParams.get("page")) : undefined;

  const [instructorsResult, searchOptionsResult] = await Promise.all([
    client.getInstructors({ search, subject, sort, page }),
    client.getSearchOptions(),
  ]);

  if (instructorsResult.isErr) {
    console.warn("Failed to load instructors:", instructorsResult.error.message);
  }
  if (searchOptionsResult.isErr) {
    console.warn("Failed to load search options:", searchOptionsResult.error.message);
  }

  return {
    instructors: instructorsResult.isOk ? instructorsResult.value : null,
    searchOptions: searchOptionsResult.isOk ? searchOptionsResult.value : null,
    url,
  };
};
