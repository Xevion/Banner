import { BannerApiClient } from "$lib/api";
import { error } from "@sveltejs/kit";
import type { PageLoad } from "./$types";

export const prerender = false;

export const load: PageLoad = async ({ params, fetch }) => {
  const client = new BannerApiClient(undefined, fetch);

  const [courseResult, searchOptionsResult] = await Promise.all([
    client.getCourse(params.term, params.crn),
    client.getSearchOptions(params.term),
  ]);

  if (courseResult.isErr) {
    if (courseResult.error.isNotFound()) {
      error(404, "Section not found");
    }
    error(500, courseResult.error.message);
  }

  const course = courseResult.value;
  const searchOptions = searchOptionsResult.isOk ? searchOptionsResult.value : null;

  return {
    course,
    searchOptions,
    term: params.term,
  };
};
